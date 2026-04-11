use std::io::{self, Write};
mod parser;

fn main() {
    loop {
        let mut regex = String::new();
        let mut text = String::new();

        print!("regex> ");
        io::stdout().flush().unwrap();
        io::stdin()
            .read_line(&mut regex)
            .expect("Failed to read line");

        if regex.ends_with('\n') {
            regex.pop();
            if regex.ends_with('\r') {
                regex.pop();
            }
        }

        print!("text> ");
        io::stdout().flush().unwrap();
        io::stdin()
            .read_line(&mut text)
            .expect("Failed to read line");

        if text.ends_with('\n') {
            text.pop();
            if text.ends_with('\r') {
                text.pop();
            }
        }

        let ast = parser::parse_regex(regex);
        println!("ast: {:?}", ast);
        let inst = compile_ast(ast);
        println!("inst: {:?}", inst);
        let r = do_match(&inst, text);
        println!("Result: {}", r);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Ast {
    Empty,
    AnyChar,
    Char(char),
    Conj(Box<Ast>, Box<Ast>),
    Disj(Box<Ast>, Box<Ast>),
    Repeat(Box<Ast>, Option<usize>, Option<usize>),
    Head,
    Tail,
}

#[derive(Debug, PartialEq, Eq)]
enum InstOp {
    Match,
    AnyChar,
    Char(char),
    Jump(usize),
    Split(usize, usize),
    PositionHead,
    PositionTail,
}

type Inst = std::vec::Vec<InstOp>;

#[derive(Clone, Copy)]
struct VMState {
    pc: usize, // "Program Counter"
    sp: usize, // "String Pointer"
}

struct VM<'a> {
    s: VMState,
    stack: std::vec::Vec<VMState>,
    inst: &'a Inst,
    text: std::vec::Vec<char>,
}

fn compile_ast(ast: Ast) -> Inst {
    let mut inst = compile_ast_sub(ast, vec![]);
    inst.push(InstOp::Match);
    inst
}

fn compile_ast_sub(ast: Ast, mut inst: Inst) -> Inst {
    use InstOp::*;
    match ast {
        Ast::Empty => {
            // inst.push(Match);
            inst
        }
        Ast::AnyChar => {
            inst.push(AnyChar);
            inst
        }
        Ast::Char(c) => {
            inst.push(Char(c));
            inst
        }
        Ast::Conj(ast1, ast2) => {
            let inst = compile_ast_sub(*ast1, inst);
            compile_ast_sub(*ast2, inst)
        }
        Ast::Disj(ast1, ast2) => {
            //     split L1, L2
            // L1: <instructions of `ast1`>
            //     jump L3
            // L2: <instructions of `ast2`>
            // L3:
            let mut inst1 = compile_ast_sub(*ast1, vec![]);
            let inst2 = compile_ast_sub(*ast2, vec![]);
            let inst_split = Split(inst.len() + 1, inst.len() + inst1.len() + 2);
            inst1.push(Jump(inst1.len() + inst2.len() + 1));

            let mut inst1 = shift_jump_index(inst1, inst.len() + 1);
            let mut inst2 = shift_jump_index(inst2, inst.len() + inst1.len() + 1);

            inst.push(inst_split);
            inst.append(&mut inst1);
            inst.append(&mut inst2);
            inst
        }
        Ast::Repeat(ast, bot, top) => {
            match (bot, top) {
                (None, None) => {
                    // The * operator
                    //
                    // L1: split L2, L3
                    // L2: <instructions of `ast`>
                    //     jump L1
                    // L3:
                    let inst1 = compile_ast_sub(*ast, vec![]);
                    let inst_split = Split(inst.len() + 1, inst.len() + inst1.len() + 2);
                    let inst_jump = Jump(inst.len());

                    let mut inst1 = shift_jump_index(inst1, inst.len() + 1);

                    inst.push(inst_split);
                    inst.append(&mut inst1);
                    inst.push(inst_jump);
                    inst
                }
                (Some(1), None) => {
                    // The + operator
                    //
                    // L1: <instructions of `ast`>
                    //     split L1, L2
                    // L2:
                    let mut inst1 = compile_ast_sub(*ast, vec![]);
                    inst1.push(Split(0, inst1.len() + 1));

                    inst.append(&mut shift_jump_index(inst1, inst.len()));
                    inst
                }
                (Some(0), Some(1)) => {
                    // The ? operator
                    //
                    //     split L1, L2
                    // L1: <instructions of `ast`>
                    // L2:
                    let inst1 = compile_ast_sub(*ast, vec![]);
                    let split = Split(inst.len() + 1, inst.len() + inst1.len() + 1);

                    let mut inst1 = shift_jump_index(inst1, inst.len() + 1);

                    inst.push(split);
                    inst.append(&mut inst1);
                    inst
                }
                _ => std::todo!(),
            }
        }
        Ast::Head => {
            inst.push(PositionHead);
            inst
            // compile_ast_sub(*ast, inst)
        }
        Ast::Tail => {
            inst.push(PositionTail);
            inst
            // compile_ast_sub(*ast, inst)
        }
    }
}

fn shift_jump_index(inst: Inst, offset: usize) -> Inst {
    inst.into_iter()
        .map(|inst| match inst {
            InstOp::Jump(label) => InstOp::Jump(label + offset),
            InstOp::Split(l1, l2) => InstOp::Split(l1 + offset, l2 + offset),
            _ => inst,
        })
        .collect::<Inst>()
}

fn do_match(inst: &Inst, text: impl Into<String>) -> bool {
    let vm = VM::new(inst, text.into());
    vm.do_match()
}

impl<'a> VM<'a> {
    pub fn new(inst: &'a Inst, text: String) -> Self {
        let text = text.chars().collect::<Vec<_>>();
        Self {
            s: VMState { pc: 0, sp: 0 },
            stack: vec![],
            inst: inst,
            text: text,
        }
    }

    pub fn do_match(mut self) -> bool {
        // self.stack.push(VMState { pc: 0, sp: 0 });
        (0..self.text.len()).for_each(|sp| self.stack.push(VMState { pc: 0, sp: sp }));
        self.stack.reverse();
        self.try_next_thread()
    }

    fn do_match_current_thread(mut self) -> bool {
        use InstOp::*;
        let op = &self.inst[self.s.pc];
        let go_next = |mut vm: Self| -> bool {
            vm.s.pc += 1;
            vm.s.sp += 1;
            vm.do_match_current_thread()
        };
        match *op {
            Match => true,
            AnyChar => match self.text.get(self.s.sp) {
                None => self.try_next_thread(),
                Some(_) => go_next(self),
            },
            Char(c) => {
                if let Some(ch) = self.text.get(self.s.sp)
                    && *ch == c
                {
                    go_next(self)
                } else {
                    self.try_next_thread()
                }
            }
            Jump(l) => {
                self.s.pc = l;
                self.do_match_current_thread()
            }
            Split(l1, l2) => {
                self.stack.push(VMState {
                    pc: l2,
                    sp: self.s.sp,
                });
                self.s.pc = l1;
                self.do_match_current_thread()
            }
            PositionHead => {
                if self.s.sp == 0 {
                    self.s.pc += 1;
                    self.do_match_current_thread()
                } else {
                    self.try_next_thread()
                }
            }
            PositionTail => {
                if self.s.sp == self.text.len() {
                    self.s.pc += 1;
                    self.do_match_current_thread()
                } else {
                    self.try_next_thread()
                }
            }
        }
    }

    fn try_next_thread(mut self) -> bool {
        if let Some(s) = self.stack.pop() {
            self.s = s;
            self.do_match_current_thread()
        } else {
            false
        }
    }
}

#[cfg(test)]
mod test_compile {
    use super::*;

    fn compile(s: impl Into<String>) -> Inst {
        compile_ast(parser::parse_regex(s))
    }

    #[test]
    fn compile_conjunction() {
        use InstOp::*;
        assert_eq!(compile("abc"), vec![Char('a'), Char('b'), Char('c'), Match]);
    }

    #[test]
    fn compile_repeat() {
        use InstOp::*;
        assert_eq!(
            compile("ab?c"),
            vec![Char('a'), Split(2, 3), Char('b'), Char('c'), Match]
        );
        assert_eq!(
            compile("a(xy)*c"),
            vec![
                Char('a'),
                Split(2, 5),
                Char('x'),
                Char('y'),
                Jump(1),
                Char('c'),
                Match
            ]
        );
        assert_eq!(
            compile("a(xy)+c"),
            vec![
                Char('a'),
                Char('x'),
                Char('y'),
                Split(1, 4),
                Char('c'),
                Match
            ]
        );
    }

    #[test]
    fn compile_disjunction() {
        use InstOp::*;
        assert_eq!(
            compile("ab|xy|12"),
            vec![
                Split(1, 8),
                Split(2, 5),
                Char('a'),
                Char('b'),
                Jump(7),
                Char('x'),
                Char('y'),
                Jump(10),
                Char('1'),
                Char('2'),
                Match
            ]
        );
    }
}

#[cfg(test)]
mod test_match {
    use super::*;

    fn test_match(regex: impl Into<String>, text: impl Into<String>) {
        let regex = regex.into();
        let text = text.into();
        let inst = compile_ast(parser::parse_regex(regex.clone()));
        assert_eq!(
            do_match(&inst, text.clone()),
            true,
            "Should match: (regex, text) = ({}, {})",
            regex,
            text
        );
    }

    fn test_not_match(regex: impl Into<String>, text: impl Into<String>) {
        let regex = regex.into();
        let text = text.into();
        let inst = compile_ast(parser::parse_regex(regex.clone()));
        assert_eq!(
            do_match(&inst, text.clone()),
            false,
            "Should NOT match: (regex, text) = ({}, {})",
            regex,
            text
        );
    }

    #[test]
    fn test_match_only_chars() {
        test_match("abc", "abc");
        test_match("a", "abc");
        test_match("bc", "abc");

        test_not_match("xyz", "abc");
        test_not_match("ax", "abc");
        test_not_match("xbc", "abc");
    }

    #[test]
    fn test_match_with_repeat_at_most_one() {
        test_match("ab?c", "ac");
        test_match("ab?c", "abc");
        test_match("ab?c", "xacx");
        test_match("(ab)?c", "c");

        test_not_match("ab?c", "abbc");
        test_not_match("^(axb)?c", "abc");
        test_not_match("^(ab)?c", "ac");
    }

    #[test]
    fn test_match_with_repeat_at_least_one() {
        test_match("ab+c", "abc");
        test_match("ab+c", "abbbbc");
        test_match("(ab)+c", "abababc");

        test_not_match("ab+c", "ac");
        test_not_match("(ab)+c", "ac");
        test_not_match("(axb)+c", "abc");
    }

    #[test]
    fn test_match_with_repeat_any() {
        test_match("ab*c", "ac");
        test_match("ab*c", "abc");
        test_match("ab*c", "abbbbc");
        test_match("(ab)*c", "abababc");

        test_not_match("(ab)*c", "axb");
        test_not_match("^(axb)*c", "abc");
    }

    #[test]
    fn test_match_head() {
        test_match("^a", "a");
        test_match("^a", "ab");
        test_not_match("^a", "ba");
    }

    #[test]
    fn test_tail() {
        test_match("a$", "a");
        test_match("a$", "xxxa");
        test_not_match("a$", "xax");
        test_not_match("a$b", "ab");
    }

    #[test]
    fn test_match_complex_regex() {
        test_match("(a?b)*c", "c");
        test_match("(a?b)*c", "bc");
        test_match("(a?b)*c", "abc");
        test_match("(a?b)*c", "abbc");
        test_match("(a?b)*c", "babbabc");
        test_match("@(x.*y)*@", "@xyxayxABCy@");
    }
}
