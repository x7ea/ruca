pub mod emit;
pub mod lex;
mod parse;
pub mod typ;

use indexmap::IndexMap;
use lex::name::Name;
use lex::tokenize;
use ordered_float::OrderedFloat as Float;

use std::hash::Hash;
use std::io::{Read, Write, stdin, stdout};
use std::process::exit;

fn main() {
    macro_rules! error {
        ($value: expr) => {
            match $value {
                Ok(value) => value,
                Err(err) => {
                    eprintln!("Error! {err}");
                    exit(1)
                }
            }
        };
    }
    let code = {
        let mut buffer = String::new();
        error!(stdin().read_to_string(&mut buffer));
        buffer.trim().to_owned()
    };
    let output = error!(Define::compile(error!(Define::parse(&code))));
    error!(stdout().write_all(output.as_bytes()));
}

impl Define {
    const LIB: [&str; 3] = ["malloc", "printf", "free"];

    pub fn compile(mut defines: Vec<Self>) -> Result<String, String> {
        let mut lib = String::new();
        let mut text = String::new();
        let ctx = &mut Context::default();

        ctx.global.lib = {
            let mut map = IndexMap::new();
            for line in Self::LIB {
                let signature = Type::Function(Box::new(Type::None), None);
                map.insert(Name::new(line)?, signature);
            }
            map
        };

        for define in &mut defines {
            if let Define::Function(name, _, _) = define {
                ctx.local.insert(name.clone(), Function::default());
            }
            define.infer(ctx)?;
        }

        for define in &defines {
            text += &define.emit(ctx)?;
        }
        let data = ctx.global.data.clone();

        for define in &defines {
            if let Define::Function(func, _, _) = define {
                ctx.global.lib.shift_remove(func);
            }
        }
        for symbol in ctx.global.lib.keys() {
            lib += &format!("\textern {symbol}\n");
        }

        Ok(format!(
            "section .data\n{data}\nsection .text\n\tglobal main\n{lib}\n{text}\n"
        ))
    }
}

// Abstract Syntax Tree (AST)

#[derive(Clone)]
pub enum Define {
    Function(Name, IndexMap<Name, Type>, Expr),
    Class(Name, IndexMap<Name, Type>),
}

#[derive(Clone, Hash, PartialEq, Debug)]
pub enum Expr {
    // Literal
    Integer(i64),
    Float(Float<f64>),
    Bool(bool),
    String(String),
    // Reference
    Variable(Name),
    Let(Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    // Memory
    Read(usize, Type, Box<Expr>),
    Write(usize, Box<Expr>, Box<Expr>),
    // Object
    New(Type),
    Access(Box<Expr>, Name),
    Nullcheck(Box<Expr>),
    Transmute(Box<Expr>, Type),
    // Structure
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    While(Box<Expr>, Box<Expr>),
    Block(Vec<Expr>),
    // Operator
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Mod(Box<Expr>, Box<Expr>),
    Eql(Box<Expr>, Box<Expr>),
    NotEq(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    GtEq(Box<Expr>, Box<Expr>),
    LtEq(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Xor(Box<Expr>, Box<Expr>),
}

#[derive(Clone, Debug, Hash)]
pub enum Type {
    String,
    Integer,
    Bool,
    Float,
    Class(Name, Option<Box<Type>>),
    Function(Box<Type>, Option<Vec<Type>>),
    None,
}

#[derive(Default, Debug)]
pub struct Context {
    global: Global,
    current: String,
    local: IndexMap<Name, Function>,
    typed: IndexMap<u64, Type>,
}

#[derive(Default, Debug)]
pub struct Global {
    idx: usize,
    data: String,
    lib: IndexMap<Name, Type>,
    vtable: IndexMap<Name, IndexMap<Name, Type>>,
}

#[derive(Default, Debug)]
pub struct Function {
    var: IndexMap<Name, Type>,
    jmp: Vec<String>,
}
