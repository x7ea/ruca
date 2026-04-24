use crate::*;

pub const SPACE: &str = " ";

impl Define {
    pub fn parse(source: &str) -> Result<Vec<Define>, String> {
        let mut result = Vec::new();
        for line in tokenize(source, "\n")? {
            macro_rules! args {
                ($args: expr) => {
                    tokenize($args, ",")?
                        .iter()
                        .map(|x| {
                            let (name, typ) = ok!(x.split_once(":"))?;
                            Ok((Name::new(name)?, Type::parse(typ)?))
                        })
                        .collect::<Result<IndexMap<Name, Type>, String>>()?
                };
            }
            if let Some(func) = line.strip_prefix("fn ") {
                let (head, body) = once!(func, SPACE)?;
                let (name, args) = ok!(ok!(head.strip_suffix(")"))?.split_once("("))?;
                result.push(Define::Function(
                    Name::new(name)?,
                    args!(args),
                    Expr::parse(&body)?,
                ));
            } else if let Some(head) = line.strip_prefix("type ") {
                let (name, args) = ok!(ok!(head.trim().strip_suffix("}"))?.split_once("{"))?;
                result.push(Define::Class(Name::new(name)?, args!(args)));
            }
        }
        Ok(result)
    }
}

impl Expr {
    pub fn parse(source: &str) -> Result<Expr, String> {
        let x = source.trim();
        macro_rules! surround {
            ($ls: literal, $x: expr, $rs: literal) => {
                $x.strip_prefix($ls).and_then(|x| x.strip_suffix($rs))
            };
            ($x: expr, $ls: literal, $rs: literal) => {
                $x.strip_suffix($rs).and_then(|x| x.split_once($ls))
            };
        }

        fn is_operator(source: &str) -> Result<(String, String, String), String> {
            let tokens: Vec<String> = tokenize(source, SPACE)?;

            if tokens.len() >= 3 {
                let pos: usize = tokens.len() - 2;
                let lhs = tokens[..pos].join(SPACE);
                let opr = tokens[pos].to_string();
                let rhs = tokens[pos + 1].to_string();
                Ok((lhs, opr, rhs))
            } else {
                Err(String::new())
            }
        }

        if let Some(x) = x.strip_prefix("let ") {
            if let Ok((name, value)) = once!(x, "=") {
                Ok(Expr::Let(
                    Box::new(Expr::parse(&name)?),
                    Box::new(Expr::parse(&value)?),
                ))
            } else {
                Err(format!("変数宣言には初期化が必要です: {source}"))
            }
        } else if let Some(x) = x.strip_prefix("if ") {
            if let Ok((cond, body)) = once!(x, "then") {
                if let Ok((then, r#else)) = once!(&body, "else") {
                    Ok(Expr::If(
                        Box::new(Expr::parse(&cond)?),
                        Box::new(Expr::parse(&then)?),
                        Some(Box::new(Expr::parse(&r#else)?)),
                    ))
                } else {
                    Ok(Expr::If(
                        Box::new(Expr::parse(&cond)?),
                        Box::new(Expr::parse(&body)?),
                        None,
                    ))
                }
            } else {
                Err(format!(
                    "If条件分岐を構文解析しましたがThen節が見つかりません: {source}"
                ))
            }
        } else if let Some(x) = x.strip_prefix("while ") {
            if let Ok((condition, loop_body)) = once!(x, "do") {
                Ok(Expr::While(
                    Box::new(Expr::parse(&condition)?),
                    Box::new(Expr::parse(&loop_body)?),
                ))
            } else {
                Err(format!(
                    "While繰り返し構文を解析しましたがDo節が見つかりません: {source}"
                ))
            }
        } else if let Some(x) = surround!("{", x, "}") {
            let mut block = vec![];
            for line in tokenize(x, "\n")? {
                let (line, _) = once!(&line, ";").unwrap_or((line, String::new()));
                if !line.trim().is_empty() {
                    block.push(Expr::parse(&line)?);
                }
            }
            Ok(Expr::Block(block))
        } else if let Ok((lhs, op, rhs)) = is_operator(x) {
            Ok(match op.as_str() {
                "+" => Expr::Add(Box::new(Expr::parse(&lhs)?), Box::new(Expr::parse(&rhs)?)),
                "-" => Expr::Sub(Box::new(Expr::parse(&lhs)?), Box::new(Expr::parse(&rhs)?)),
                "*" => Expr::Mul(Box::new(Expr::parse(&lhs)?), Box::new(Expr::parse(&rhs)?)),
                "/" => Expr::Div(Box::new(Expr::parse(&lhs)?), Box::new(Expr::parse(&rhs)?)),
                "%" => Expr::Mod(Box::new(Expr::parse(&lhs)?), Box::new(Expr::parse(&rhs)?)),
                "==" => Expr::Eql(Box::new(Expr::parse(&lhs)?), Box::new(Expr::parse(&rhs)?)),
                "!=" => Expr::NotEq(Box::new(Expr::parse(&lhs)?), Box::new(Expr::parse(&rhs)?)),
                ">" => Expr::Gt(Box::new(Expr::parse(&lhs)?), Box::new(Expr::parse(&rhs)?)),
                "<" => Expr::Lt(Box::new(Expr::parse(&lhs)?), Box::new(Expr::parse(&rhs)?)),
                ">=" => Expr::GtEq(Box::new(Expr::parse(&lhs)?), Box::new(Expr::parse(&rhs)?)),
                "<=" => Expr::LtEq(Box::new(Expr::parse(&lhs)?), Box::new(Expr::parse(&rhs)?)),
                "&" => Expr::And(Box::new(Expr::parse(&lhs)?), Box::new(Expr::parse(&rhs)?)),
                "|" => Expr::Or(Box::new(Expr::parse(&lhs)?), Box::new(Expr::parse(&rhs)?)),
                "^" => Expr::Xor(Box::new(Expr::parse(&lhs)?), Box::new(Expr::parse(&rhs)?)),
                op => return Err(format!("不明な演算子: {op}")),
            })
        } else if let Some(class) = x.strip_suffix("?") {
            Ok(Expr::Nullcheck(Box::new(Expr::parse(class)?)))
        } else if let Some(_) = surround!("\"", x, "\"") {
            Ok(Expr::String(x.to_owned()))
        } else if let Some(expr) = surround!("(", x, ")") {
            Expr::parse(expr)
        } else if let Some((func, args)) = surround!(x, "(", ")") {
            Ok(Expr::Call(
                Box::new(Expr::parse(&func)?),
                tokenize(&args, ",")?
                    .iter()
                    .map(|x| Expr::parse(x))
                    .collect::<Result<Vec<_>, String>>()?,
            ))
        } else if let Ok(literal) = x.parse::<bool>() {
            Ok(Expr::Bool(literal))
        } else if let Ok(literal) = x.parse::<i64>() {
            Ok(Expr::Integer(literal))
        } else if let Ok(literal) = x.parse::<f64>() {
            use ordered_float::OrderedFloat;
            Ok(Expr::Float(OrderedFloat(literal)))
        } else if let Some((object, property)) = x.rsplit_once(".") {
            Ok(Expr::Access(
                Box::new(Expr::parse(object)?),
                Name::new(property)?,
            ))
        } else if let Some(class) = x.strip_prefix("new ") {
            Ok(Expr::New(Type::parse(&class)?))
        } else {
            Ok(Expr::Variable(Name::new(x)?))
        }
    }
}

impl Type {
    pub fn parse(source: &str) -> Result<Type, String> {
        match source.trim() {
            "Int" => Ok(Type::Integer),
            "Str" => Ok(Type::String),
            "Bool" => Ok(Type::Bool),
            "Float" => Ok(Type::Float),
            name => {
                if let Some((class, arg)) = name.split_once("@") {
                    Ok(Type::Class(
                        Name::new(class)?,
                        Some(Box::new(Type::parse(arg)?)),
                    ))
                } else if let Some((class, args)) =
                    name.strip_suffix(")").and_then(|x| x.split_once("("))
                {
                    Ok(Type::Function(
                        Box::new(Type::parse(class)?),
                        Some(
                            tokenize(args, ",")?
                                .iter()
                                .map(|x| Type::parse(&x))
                                .collect::<Result<Vec<Type>, String>>()?,
                        ),
                    ))
                } else {
                    Ok(Type::Class(Name::new(name)?, None))
                }
            }
        }
    }
}

impl PartialEq for Type {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Type::Integer, Type::Integer)
            | (Type::Float, Type::Float)
            | (Type::String, Type::String)
            | (Type::Bool, Type::Bool) => true,
            (Type::Function(ret1, arg1), Type::Function(ret2, arg2)) => {
                ret1 == ret2 && arg1 == arg2
            }
            (Type::Class(name1, _), Type::Class(name2, _)) => name1 == name2,
            _ => false,
        }
    }
}
