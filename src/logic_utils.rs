use std::collections::VecDeque;

use crate::str_utils::{StrUtils};
use crate::table::{TableDataGetter, TableData, ColumnGetter};
use regex::{Regex};

#[derive(Clone)]
pub struct LiteralValue {
    value_as_string: Option<String>,
    value_as_float: Option<f32>,
    value_as_int: Option<usize>,
    value_as_str_array: Option<Vec<String>>,
}

pub enum Item {
    Literal(LiteralValue),
    Token(String),
    Operator(char),
    ZoneStart(char),
    ZoneEnd(char),
}

impl Item {
    fn get_literal_as_number(&self) -> usize {
        if let Item::Literal(value)= self {
            return value.value_as_int.expect("Expected literal number")
        } else {
            panic!("Current item should be a literal");
        }
    }

    pub fn conduct_string_literal(val: String) -> Item {
        return Item::Literal(Item::conduct_str_literal_value(val));
    }

    pub fn conduct_float_literal(val: f32) -> Item {
        return Item::Literal(Item::conduct_float_literal_value(val));
    }

    pub fn conduct_int_literal_value(val: usize) -> LiteralValue {
        return LiteralValue {
            value_as_string: None,
            value_as_str_array: None,
            value_as_float: None,
            value_as_int: Some(val),
        }
    }

    pub fn conduct_float_literal_value(val: f32) -> LiteralValue {
        return LiteralValue {
            value_as_string: None,
            value_as_str_array: None,
            value_as_float: Some(val),
            value_as_int: None,
        }
    }

    pub fn conduct_str_literal_value(val: String) -> LiteralValue {
        return LiteralValue {
            value_as_string: Some(val.to_string()),
            value_as_str_array: None,
            value_as_float: None,
            value_as_int: None,
        }
    }

    pub fn conduct_int_literal(val: usize) -> Item {
        return Item::Literal(Item::conduct_int_literal_value(val));
    }

    fn get_end_zone_character(&self) -> char {
        if let Item::ZoneEnd(s) = self {
            *s
        } else {
            panic!("Current item should be a literal");
        }
    }

    fn get_as_operator(&self) -> char {
        if let Item::Operator(s) = self {
            *s
        } else {
            panic!("Current item should be a literal");
        }
    }

    fn get_literal_as_text(&self) -> String {
        if let Item::Literal(value) = self {
            return value.value_as_string.clone().unwrap();
        } else {
            panic!("Current item should be a literal");
        }
    }

    fn get_literal(&self) -> &LiteralValue {
        if let Item::Literal(value) = self {
            return value;
        } else {
            panic!("Current item should be a literal");
        }
    }

    fn get_token(&self) -> String {
        if let Item::Token(s) = self {
            return String::from(s);
        } else {
            panic!("Current item should be a token");
        }
    }

    fn expect_start_of(&self, char_value: char) {
        if let Item::ZoneStart(s) = self {
            assert_eq!(s, &char_value, "Column reference should preceed <");
        } else {
            panic!("Current item should be literal");
        }
    }
}

pub trait LogicExecutor {
    fn parse_string(&self, s: String, index: u32, name: String) -> String;
    // fn get_command(s: &str) -> Command;
    fn execute_str(&self, s: &str, index: u32, name: String) -> String;
    fn fill_data(&mut self);
    fn revaluate_from_end_zone(&self, stack: &mut VecDeque<Item>, inc_from: &mut usize);
    fn calc_function(&self, name: &str, args: Vec<LiteralValue>, inc_from: &mut usize) -> LiteralValue;
    fn evaluate_arithmetic(&self, operator: char, args: &Vec<LiteralValue>) -> LiteralValue;
    fn resolve_literal_at(&self, s: &str, i: usize, current_row_number: u32) -> (String, usize);
    fn revaluate_from_literal(&self, stack: &mut VecDeque<Item>);
    fn increase_column_digits(text: String, prev_num: u32) -> String;
    fn evaluate_column_reference(&self, stack: &mut VecDeque<Item>);
    fn evaluate_curly_zone(&self, stack: &mut VecDeque<Item>, inc_from: &mut usize);
    fn inc_from(&self, args: Vec<LiteralValue>, inc_from: &mut usize) -> LiteralValue;
    fn split(&self, args: Vec<LiteralValue>) -> LiteralValue;
}

impl LogicExecutor for TableData {
    fn increase_column_digits(text: String, prev_num: u32) -> String {
        return Regex::new(format!("[A-Z]{prev_num}+").as_str())
            .unwrap()
            .replace_all(&text, |caps: &regex::Captures| {
                caps[0][0..=0].to_string() + &(prev_num + 1).to_string()
            }).to_string();
    }

    fn revaluate_from_literal(&self, stack: &mut VecDeque<Item>) {}

    fn evaluate_column_reference(&self, stack: &mut VecDeque<Item>) {
        let column_with_index = stack.pop_back().expect("column reference should predicate index");
        let column_index = column_with_index.get_literal_as_number();
        stack
            .pop_back()
            .expect("Column ref should precede index")
            .expect_start_of('<');
        let column_name = stack
            .pop_back()
            .expect("Column ref should precede name")
            .get_token();
        let res = String::from(
            self
                .get_by_name_unmut(column_name.as_str().remove_first_symbol()) // drop @
                .get_cell_by_index(column_index));
        stack.push_back(Item::conduct_string_literal(res));
    }

    fn evaluate_curly_zone(&self, stack: &mut VecDeque<Item>, inc_from: &mut usize) {
        let mut operands: Vec<LiteralValue> = vec![];
        loop {
            let item_inner = stack.pop_back().expect("No matching pair of ')' found");
            if let Item::Literal(value) = item_inner {
                operands.push(value);
            } else if let Item::ZoneStart(value) = item_inner {
                assert_eq!(value, '(', "no opening braces");
                //         3 possible cases by now
                //   =E^v+(E^v*A9) | split(D2, ",") | (2+3)
                //       +               +            +
                if !stack.is_empty() {
                    let item_before_braces = stack.pop_back().unwrap();
                    if let Item::Token(operation) = item_before_braces {
                        let res = self.calc_function(&operation, operands, inc_from);
                        stack.push_back(Item::Literal(res));
                        break;
                    }
                    // if it's not a token, than we should return it to stack
                    // so we don't break the structure
                    stack.push_back(item_before_braces);
                }
                if operands.len() == 1 {
                    stack.push_back(Item::Literal(operands[0].clone()))
                } else {
                    panic!("Multiple operands withing braces without operation")
                }
                break;
            // part of arithmetic operations, validate the last one,
            // and queue back to the stack in case there are multiple of them
            // e.g. 2+3+4+6
            } else if let Item::Operator(operator) = item_inner {
                let left_operand = stack
                    .pop_back()
                    .expect("No left expression to operator")
                    .get_literal().clone();
                operands.push(left_operand);
                let res = self.evaluate_arithmetic(operator, &operands);
                operands.clear();
                stack.push_back(Item::Literal(res));
            } else {
                panic!("Invalid operation")
            }
        }
    }

    fn revaluate_from_end_zone(&self, stack: &mut VecDeque<Item>, inc_from: &mut usize) {
        let end_zone_symbol = stack
            .pop_back()
            .unwrap()
            .get_end_zone_character();

        if end_zone_symbol == '>' {  // @adjusted_cost<1>
            self.evaluate_column_reference(stack);
        } else { // split(D2, ",") ||||||    (E^v*A9)
            self.evaluate_curly_zone(stack, inc_from);
        }
    }

    fn calc_function(&self, name: &str, args: Vec<LiteralValue>, inc_from: &mut usize) -> LiteralValue {
        if (name == "incFrom") {
            return self.inc_from(args, inc_from);
        } else if (name == "split") {
            return self.split(args);
        }
        return Item::conduct_str_literal_value(format!("[{}({})]", name, "wtf"));
    }

    fn inc_from(&self, args: Vec<LiteralValue>, inc_from: &mut usize) -> LiteralValue {
        assert_eq!(args.len(), 1, "incFrom accept 1 arg");
        let i = args[0].value_as_int.unwrap();
        *inc_from =  *inc_from + i;
        return Item::conduct_int_literal_value(*inc_from);
    }

    fn split(&self, args: Vec<LiteralValue>) -> LiteralValue {
        assert_eq!(args.len(), 2, "split accept 2 arg");
        let from_column = args[0].value_as_string.clone().unwrap();
        let separator = args[1].value_as_string.clone().unwrap();
        assert_eq!(separator.len(), 1, "separator should be 1 symbol");
        let x: char = separator.at(0);
        let array_strs = from_column
            .as_str()
            .split(x)
            .map(|x| String::from(x))
            .collect::<Vec<String>>();
        let value = LiteralValue {
            value_as_string: None,
            value_as_float: None,
            value_as_int: None,
            value_as_str_array: Some(array_strs)
        };
        return value;
    }

    fn evaluate_arithmetic(&self, operator: char, args: &Vec<LiteralValue>) -> LiteralValue {
        assert_eq!(args.len(), 2, "Cannot operate with complex arguments");
        let a = args[0].value_as_float.unwrap();
        let b = args[1].value_as_float.unwrap();
        let c =  match operator {
            '+' => a+b,
            '-' => a-b,
            '*' => a*b,
            '/' => a/b,
            _ => panic!("WTf")
        };
        return Item::conduct_float_literal_value(c);
    }

    fn resolve_literal_at(&self, s: &str, i: usize, current_row_number: u32) -> (String, usize) {
        if s.at(i + 1).is_ascii_digit() { // cell reference
            let index = &s.at(i + 1).to_string().parse::<u32>().expect("Expected literal number");
            let value = self.get_by_coordinate(s.at(i), index);
            return (value, 2);
        } else if &s[i + 1..=i + 2] == "^v" {
            let res = self.get_last_value_of_the_column(s.at(i));
            return (String::from(res), 3);
        } else if &s[i + 1..=i + 1] == "^" {
            let value = self.get_by_coordinate(s.at(i), &(current_row_number - 1));
            return (value, 2);
        } else {
            panic!("Unsupported structure for value {}", &s[i + 1..=i + 1]);
        }
    }

    fn execute_str(&self, s: &str, index: u32, name: String) -> String {
        let mut stack: VecDeque<Item> = VecDeque::new();
        let mut inc_from: usize = 0; // for incFrom(
        let mut i = 0;
        while i < s.len() {
            // if resolved to literal
            if s.at(i).is_uppercase() && !s.at(i + 1).is_alphabetic() {
                let (literal, literal_length) = self.resolve_literal_at(s, i, index);
                i += literal_length;
                stack.push_back(Item::conduct_string_literal(literal))
                // if token
            } else if s.at(i).is_ascii_alphabetic() {
                let token_length = s.next_word_length(i + 1);
                let token = s[i..i + token_length + 1].to_string();
                stack.push_back(Item::Token(token));
                i += token_length + 1;
                // if number
            } else if s.at(i).is_ascii_digit() {
                let digit_length = s.next_digit_length(i + 1);
                let digit = &s[i..i + digit_length + 1];
                let digitValue = digit.to_string().parse::<usize>().unwrap();
                stack.push_back(Item::conduct_int_literal(digitValue));
                i += digit_length + 1;
                // if expression starts
            } else if ['(', '<'].contains(&s.at(i)) {
                stack.push_back(Item::ZoneStart(s.at(i)));
                i += 1;
                // if expression end, here we need to evaluated all inside of it.
            } else if [')', '>'].contains(&s.at(i)) {
                stack.push_back(Item::ZoneEnd(s.at(i)));
                i += 1;
                self.revaluate_from_end_zone(&mut stack, &mut inc_from);
                // if string literal
            } else if s.at(i) == '\"' {
                let literal_length = s.next_quote_length(i + 1);
                let val = &s[i..i+1 + literal_length + 1]; // 2 is start+end quote
                stack.push_back(Item::conduct_string_literal(val.to_string()));
                i += literal_length + 2; // "asd" = 3 + 2
                // if column reference
            } else if s.at(i) == '@' {
                let token_length = s.next_word_length_underscore(i + 1);
                stack.push_back(Item::Token(s[i..i + token_length + 1].to_string()));
                i += token_length + 1; // @ + word itself
                // ignore separators
            } else if [' ', ','].contains(&s.at(i)) {
                i += 1;
                // arithmetic operations
            } else if ['+', '*', '-', '/'].contains(&s.at(i)) {
                stack.push_back(Item::Operator(s.at(i)));
                i += 1;
            } else {
                panic!("Unknown symbol {} at position {}", &s[i..i + 1], i);
            }
        }
        while !stack.is_empty() {
            if stack.len() == 1 {
                return stack
                    .pop_back()
                    .expect("Stack evaluated to 0")
                    .get_literal_as_text();
            }
            stack.push_front(Item::ZoneStart('('));
            self.evaluate_curly_zone(&mut stack, &mut inc_from);
        }
        panic!("Invalid expression");
    }

    fn parse_string(&self, s: String, index: u32, name: String) -> String {
        return if &s.as_str()[0..=0] == "=" {
            // this formula should be evaluated
            self.execute_str(s.as_str().remove_first_symbol(), index, name)

        } else {
            s
        };
    }

    fn fill_data(&mut self) {
        for column_index in 0..self.columns.len() {
            let row_keys: Vec<u32> = self.columns[column_index].get_sorted_keys();
            let mut prev_val: String = String::from("");
            for row_number in row_keys {
                let cell: &String = self.columns[column_index].values.get(&row_number).unwrap();
                // this formula should be resolved before the main loop, since it allows only 1 expression
                if cell == "=^^" {
                    // replace occurrences in the current row
                    // sum(spread(split(D2, ","))) -> sum(spread(split(D3, ",")))
                    let s = TableData::increase_column_digits(prev_val.clone(), row_number - 1);
                    let calculated_data = self.parse_string(
                        s,
                        row_number,
                        String::from(&self.columns[column_index].name),
                    );
                    self.columns[column_index].values.insert(row_number, calculated_data);
                } else {
                    prev_val = String::from(cell);
                    let calculated_data = self.parse_string(
                        String::from(cell),
                        row_number,
                        String::from(&self.columns[column_index].name),
                    );
                    self.columns[column_index].values.insert(row_number, calculated_data);
                }
            }
        }
    }
}
