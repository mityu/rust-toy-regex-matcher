// re        := disj
// disj      := conj | conj '|' disj
// conj      := postfixed | postfixed conj
// postfixed := atom | atom postfix
// atom      := alpha | '(' re ')'
// postfix   := '*' | '+' | '?'

use crate::Ast;
use std::iter::{Iterator, Peekable};

pub fn parse_regex(regex: impl Into<String>) -> Ast {
    let regex = regex.into();
    let iter = regex.chars().peekable();
    let (mut iter, ast) = parse_exp(iter);
    if let Some(_) = iter.next() {
        std::unreachable!();
    }
    ast
}

fn parse_exp<I>(regex: Peekable<I>) -> (Peekable<I>, Ast)
where
    I: Iterator<Item = char>,
{
    /*
    use Ast::*;
    let c = regex.next();
    if let Some(c) = c {
        // Broken
        match c {
            '.' => parse_exp(regex, Conj(Box::new(ast), Box::new(AnyChar))),
            '*' => parse_exp(regex, Repeat(Box::new(ast), None, None)),
            '?' => parse_exp(regex, Repeat(Box::new(ast), Some(0), Some(1))),
            '+' => parse_exp(regex, Repeat(Box::new(ast), Some(1), None)),
            '|' => {
                let (regex, ast2) = parse_exp(regex, Ast::Empty);
                parse_exp(regex, Disj(Box::new(ast), Box::new(ast2)))
            }
            '^' => parse_exp(regex, Head(Box::new(ast))),
            '$' => parse_exp(regex, Tail(Box::new(ast))),
            '(' => std::todo!(),
            '\\' => {
                let c = match regex.next() {
                    None => '\\',
                    Some(c) => match c {
                        '*' | '?' | '+' | '|' | '\\' | '^' | '$' | '(' => c,
                        _ => panic!("Invalid character after \\: '{}'", c),
                    },
                };
                parse_exp(regex, Conj(Box::new(ast), Box::new(Char(c))))
            }
            c => parse_exp(regex, Conj(Box::new(ast), Box::new(Char(c)))),
        }
    } else {
        (regex, ast)
    }
    */
    /*
    let mut ast = Ast::Empty;
    while let Some(_) = regex.peek() {
        let (re, infixed) = parse_infixed(regex);
        regex = re;
        if let Some(')') = regex.peek() {
            // The token ')' will be consumed in `parse_atom`.
            return (regex, ast);
        }
        ast = Ast::Conj(Box::new(ast), Box::new(infixed));
    }
    (regex, ast)
    */
    parse_disjunction(regex, Ast::Empty)
}

// fn parse_infixed<I>(regex: Peekable<I>) -> (Peekable<I>, Ast)
// where
//     I: Iterator<Item = char>,
// {
//     let (mut regex, postfixed) = parse_postfixed(regex);
//     if let Some(c) = regex.next_if(|&c| is_infix_op(c)) {
//         if c != '|' {
//             std::unreachable!();
//         }
//         let (regex, infixed) = parse_infixed(regex);
//         (regex, Ast::Disj(Box::new(postfixed), Box::new(infixed)))
//     } else {
//         (regex, postfixed)
//     }
// }

fn parse_disjunction<I>(mut regex: Peekable<I>, ast: Ast) -> (Peekable<I>, Ast)
where
    I: Iterator<Item = char>,
{
    match regex.peek() {
        None => (regex, ast),
        Some(')') => (regex, ast), // The character ')' is consumed by `parse_atom`.
        Some('|') => {
            regex.next(); // Consume the '|'.
            let (regex, conjunction) = parse_conjunction(regex, Ast::Empty);
            parse_disjunction(regex, Ast::Disj(Box::new(ast), Box::new(conjunction)))
        }
        _ => {
            let (regex, ast) = parse_conjunction(regex, ast);
            parse_disjunction(regex, ast)
        }
    }
}

fn parse_conjunction<I>(mut regex: Peekable<I>, ast: Ast) -> (Peekable<I>, Ast)
where
    I: Iterator<Item = char>,
{
    match regex.peek() {
        None | Some(')') | Some('|') => (regex, ast),
        _ => {
            let (regex, postfixed) = parse_postfixed(regex);
            let ast = if ast == Ast::Empty {
                postfixed
            } else {
                Ast::Conj(Box::new(ast), Box::new(postfixed))
            };
            parse_conjunction(regex, ast)
        }
    }
}

fn parse_postfixed<I>(regex: Peekable<I>) -> (Peekable<I>, Ast)
where
    I: Iterator<Item = char>,
{
    let (mut regex, atom) = parse_atom(regex);
    if let Some(c) = regex.next_if(|&c| is_postfix_op(c)) {
        match c {
            '*' => (regex, Ast::Repeat(Box::new(atom), None, None)),
            '+' => (regex, Ast::Repeat(Box::new(atom), Some(1), None)),
            '?' => (regex, Ast::Repeat(Box::new(atom), Some(0), Some(1))),
            _ => std::unreachable!(),
        }
    } else {
        (regex, atom)
    }
}

fn parse_atom<I>(mut regex: Peekable<I>) -> (Peekable<I>, Ast)
where
    I: Iterator<Item = char>,
{
    if let Some(c) = regex.next_if(|&c| !(is_infix_op(c) || is_postfix_op(c))) {
        match c {
            '.' => (regex, Ast::AnyChar),
            '^' => (regex, Ast::Head),
            '$' => (regex, Ast::Tail),
            '(' => {
                let (mut regex, ast) = parse_exp(regex);
                if let Some(')') = regex.next() {
                    (regex, ast)
                } else {
                    panic!("Unmatched '('")
                }
            }
            ')' => panic!("Unmatched ')'"),
            '\\' => match regex.next() {
                None => (regex, Ast::Char('\\')),
                Some(c) => match c {
                    '.' | '^' | '$' | '(' | ')' | '|' => (regex, Ast::Char(c)),
                    _ => panic!("Invalid character after \\: '{}'", c),
                },
            },
            _ => (regex, Ast::Char(c)),
        }
    } else {
        (regex, Ast::Empty)
    }
}

fn is_postfix_op(c: char) -> bool {
    match c {
        '*' | '+' | '?' => true,
        _ => false,
    }
}

fn is_infix_op(c: char) -> bool {
    match c {
        '|' => true,
        _ => false,
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_single_char() {
        use Ast::*;
        assert_eq!(Char('a'), parse_regex("a"));
        assert_eq!(AnyChar, parse_regex("."));
    }

    #[test]
    fn parse_regex_with_question() {
        use Ast::*;
        let new = Box::new;
        assert_eq!(
            Conj(
                new(Conj(
                    new(Char('a')),
                    new(Repeat(new(Char('b')), Some(0), Some(1)))
                )),
                new(Char('c'))
            ),
            parse_regex("ab?c")
        )
    }

    #[test]
    fn parse_escaped_char() {
        use Ast::*;
        assert_eq!(Char('.'), parse_regex("\\."));
        assert_eq!(Char('('), parse_regex("\\("));
        assert_eq!(Char(')'), parse_regex("\\)"));
    }

    #[test]
    fn parse_regex_with_branch() {
        use Ast::*;
        let new = Box::new;
        assert_eq!(
            Disj(
                new(Conj(new(Char('a')), new(Char('b')))),
                new(Conj(new(Char('x')), new(Char('y'))))
            ),
            parse_regex("ab|xy")
        );
        assert_eq!(
            Disj(new(Empty), new(Conj(new(Char('a')), new(Char('b'))))),
            parse_regex("|ab")
        );
        assert_eq!(
            Disj(
                new(Disj(new(Disj(new(Char('a')), new(Char('b')))), new(Empty))),
                new(Char('c'))
            ),
            parse_regex("a|b||c")
        );
    }

    #[test]
    #[should_panic]
    fn parse_should_panic_for_mismatched_opening_parenthesis() {
        parse_regex("a(xyz");
    }

    #[test]
    #[should_panic]
    fn parse_should_panic_for_mismatched_closing_parenthesis() {
        parse_regex("abc)");
    }
}
