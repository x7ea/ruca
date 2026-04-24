use crate::*;

pub const ABI: [&str; 6] = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"];

impl Define {
    pub fn emit(&self, ctx: &mut Context) -> Result<String, String> {
        let Define::Function(name, args, body) = self else {
            return Ok(String::new());
        };

        let mut addr = 8usize;
        let mut prologue = String::new();

        let mut idx = 0;
        let mut xmm = 0;
        for (count, (_, typ)) in args.iter().enumerate() {
            if let Type::Float = typ {
                prologue += &(if xmm < 8 {
                    format!("\tmovsd [rbp-{addr}], xmm{xmm}\n")
                } else {
                    format!(
                        "\tmovsd xmm0, [rbp+{}]\n\tmovsd [rbp-{addr}], xmm0\n",
                        (count - 4) * 8
                    )
                });
                xmm += 1;
            } else {
                prologue += &(if let Some(reg) = ABI.get(idx) {
                    format!("\tmov [rbp-{addr}], {reg}\n")
                } else {
                    format!(
                        "\tmov rax, [rbp+{}]\n\tmov [rbp-{addr}], rax\n",
                        (count - 4) * 8
                    )
                });
                idx += 1;
            }
            addr += 8;
        }

        ctx.current = name.to_string();
        let body = body.emit(ctx)?;
        let size = ctx.local().var.len() * 8;

        Ok(format!(
            "{name}:\n\tpush rbp\n\tmov rbp, rsp\n\tsub rsp, {}\n{prologue}{body}\tleave\n\tret\n\n",
            if size % 16 == 0 { size } else { size + 8 }
        ))
    }
}

impl Expr {
    fn emit(&self, ctx: &mut Context) -> Result<String, String> {
        macro_rules! op {
            ($asm: literal, $lhs: expr, $rhs: expr) => {
                match ctx.typed.get(&hash!(self)).unwrap() {
                    Type::Integer | Type::Bool => format!(
                        "{}\tpush rax\n{}\tmov r10, rax\n\tpop rax\n\t{} rax, r10\n",
                        $lhs.emit(ctx)?, $rhs.emit(ctx)?, $asm,
                    ),
                    Type::Float => format!(
                        "{}\tsub rsp, 8\n\tmovsd [rsp], xmm0\n{}\tmovsd xmm1, xmm0\n\tmovsd xmm0, [rsp]\n\tadd rsp, 8\n\t{}sd xmm0, xmm1\n",
                        $lhs.emit(ctx)?, $rhs.emit(ctx)?, $asm.replace("imul", "mul"),
                    ),
                    _ => panic!()
                }
            };
        }
        macro_rules! cmp {
            ($op: literal, $lhs: expr , $rhs: expr) => {
                format!(
                    "{}\tset{} al\n\tmovzx rax, al\n",
                    op!("cmp", $lhs, $rhs),
                    $op
                )
            };
        }
        macro_rules! class {
            ($object: expr) => {{
                let Type::Class(class, arg) = $object else {
                    panic!()
                };
                (class, arg)
            }};
        }
        macro_rules! label {
            () => {{
                let id = ctx.global.idx;
                ctx.global.idx += 1;
                id.to_string()
            }};
        }

        match self {
            Expr::If(cond, then, els) => {
                let id = label!();
                if let Some(els) = els {
                    Ok(format!(
                        "{}\tcmp rax, 0\n\tje else.{id}\n{}\tjmp if.{id}\nelse.{id}:\n{}if.{id}:\n",
                        cond.emit(ctx)?,
                        then.emit(ctx)?,
                        els.emit(ctx)?,
                    ))
                } else {
                    Ok(format!(
                        "{}\tcmp rax, 0\n\tje if.{id}\n{}if.{id}:\n",
                        cond.emit(ctx)?,
                        then.emit(ctx)?,
                    ))
                }
            }
            Expr::While(cond, body) => {
                let id = label!();
                ctx.local().jmp.push(id.clone());
                let output = format!(
                    "while.{id}:\n{}\tcmp rax, 0\n\tje do.{id}\n{}\tjmp while.{id}\ndo.{id}:\n",
                    cond.emit(ctx)?,
                    body.emit(ctx)?,
                );
                ctx.local().jmp.pop();
                Ok(output)
            }
            Expr::Block(lines) => Ok(lines
                .iter()
                .map(|line| line.emit(ctx))
                .collect::<Result<String, String>>()?),
            Expr::Call(callee, args) => {
                let mut push = String::new();
                let mut mov = String::new();

                macro_rules! is_float {
                    ($arg: expr) => {
                        ctx.typed.get(&hash!($arg.clone())).unwrap() == &Type::Float
                    };
                }

                for arg in args.iter().rev() {
                    push += &arg.emit(ctx)?;
                    push += if is_float!(arg) {
                        "\tsub rsp, 8\n\tmovsd [rsp], xmm0\n"
                    } else {
                        "\tpush rax\n"
                    };
                }

                let mut idx = 0;
                let mut xmm = 0;
                for arg in args.iter() {
                    if is_float!(arg) {
                        if xmm < 8 {
                            mov += &format!("\tmovsd xmm{xmm}, [rsp]\n\tadd rsp, 8\n");
                        }
                        xmm += 1;
                    } else {
                        if let Some(reg) = ABI.get(idx) {
                            mov += &format!("\tpop {reg}\n");
                        }
                        idx += 1;
                    }
                }

                Ok(format!(
                    "{push}{mov}{}\tmov r10, rax\n\tmov rax, {xmm}\n\tcall r10\n",
                    callee.emit(ctx)?
                ))
            }
            Expr::Variable(name) => {
                let env = &ctx.local().var;
                if let Some(i) = env.get_index_of(name) {
                    let typ = env.get(name).unwrap();
                    let addr = (i + 1) * 8;
                    if let Type::Float = typ {
                        Ok(format!("\tmovsd xmm0, [rbp-{addr}]\n"))
                    } else {
                        Ok(format!("\tmov rax, [rbp-{addr}]\n"))
                    }
                } else {
                    Ok(format!("\tlea rax, [{name}]\n"))
                }
            }
            Expr::Let(name, value) => match &**name {
                Expr::Variable(name) => {
                    let env = &mut ctx.local().var;
                    let idx = env.get_index_of(name).unwrap();
                    let typ = env.get(name).unwrap().clone();

                    let (value, addr) = (value.emit(ctx)?, (idx + 1) * 8);
                    if let Type::Float = typ {
                        Ok(format!("{value}\tmovsd [rbp-{addr}], xmm0\n"))
                    } else {
                        Ok(format!("{value}\tmov [rbp-{addr}], rax\n"))
                    }
                }
                Expr::Access(object, property) => {
                    let (class_name, _) = class!(object.infer(ctx)?);
                    let layout = ctx.global.vtable.get(&class_name).unwrap().clone();
                    Expr::Write(
                        layout.get_index_of(property).unwrap(),
                        value.clone(),
                        object.clone(),
                    )
                    .emit(ctx)
                }
                _ => panic!(),
            },
            Expr::New(class) => {
                let (class_name, _) = class!(class);
                let layout = ctx.global.vtable.get(class_name).unwrap();
                initializer!(layout).emit(ctx)
            }
            Expr::Access(object, property) => {
                let (class_name, _) = class!(object.infer(ctx)?);
                let layout = ctx.global.vtable.get(&class_name).unwrap().clone();
                Expr::Read(
                    layout.get_index_of(property).unwrap(),
                    ctx.typed.get(&hash!(self)).unwrap().clone(),
                    object.clone(),
                )
                .emit(ctx)
            }
            Expr::Nullcheck(expr) => {
                class!(expr.infer(ctx)?);
                let expr = Expr::NotEq(
                    Box::new(Expr::Transmute(expr.clone(), Type::Integer)),
                    Box::new(Expr::Integer(0)),
                );
                expr.infer(ctx)?;
                expr.emit(ctx)
            }
            Expr::Transmute(expr, _typ) => expr.emit(ctx),
            Expr::Read(offset, typ, addr) => {
                let calc = format!("[rax+{}]", offset * 8);
                let addr = addr.emit(ctx)?;

                Ok(format!(
                    "{addr}\tpxor xmm0, xmm0\n\tcmp rax, 0\n\tje null.{id}\n\tlea rax, {calc}\n{}null.{id}:\n",
                    if let Type::Float = typ {
                        "\tmovsd xmm0, [rax]\n"
                    } else {
                        "\tmov rax, [rax]\n"
                    },
                    id = label!()
                ))
            }
            Expr::Write(offset, value, addr) => {
                let calc = format!("[rax+{}]", offset * 8);
                let [addr, value] = [addr.emit(ctx)?, value.emit(ctx)?];

                Ok(format!(
                    "{addr}\tpxor xmm0, xmm0\n\tcmp rax, 0\n\tje null.{id}\n\tlea r11, {calc}\n\tpush r11\n{value}\tpop r11\n{}null.{id}:\n",
                    if let Type::Float = self.infer(ctx)?.clone() {
                        "\tmovsd [r11], xmm0\n"
                    } else {
                        "\tmov [r11], rax\n"
                    },
                    id = label!()
                ))
            }
            Expr::Integer(value) => Ok(format!("\tmov rax, {value}\n")),
            Expr::Bool(value) => Expr::Integer(if *value { 1 } else { 0 }).emit(ctx),
            Expr::Float(value) => {
                let name = format!("float.{}", label!());
                ctx.global.data += &format!("\t{name} dq {value:?}\n");
                Ok(format!("\tmovsd xmm0, [{name}]\n"))
            }
            Expr::String(value) => {
                let value = format!("{value}, 0")
                    .replace("\\n", "\", 10, \"")
                    .replace("\\\"", "\", 34, \"")
                    .replace("\"\", ", "");

                let name = format!("str.{}", label!());
                ctx.global.data += &format!("\t{name} db {value}\n");

                Ok(format!("\tmov rax, {name}\n"))
            }
            Expr::Add(lhs, rhs) => Ok(op!("add", lhs, rhs)),
            Expr::Sub(lhs, rhs) => Ok(op!("sub", lhs, rhs)),
            Expr::Mul(lhs, rhs) => Ok(op!("imul", lhs, rhs)),
            Expr::Eql(lhs, rhs) => Ok(cmp!("e", lhs, rhs)),
            Expr::NotEq(lhs, rhs) => Ok(cmp!("ne", lhs, rhs)),
            Expr::Gt(lhs, rhs) => Ok(cmp!("g", lhs, rhs)),
            Expr::Lt(lhs, rhs) => Ok(cmp!("l", lhs, rhs)),
            Expr::GtEq(lhs, rhs) => Ok(cmp!("ge", lhs, rhs)),
            Expr::LtEq(lhs, rhs) => Ok(cmp!("le", lhs, rhs)),
            Expr::And(lhs, rhs) => Ok(op!("and", lhs, rhs)),
            Expr::Or(lhs, rhs) => Ok(op!("or", lhs, rhs)),
            Expr::Xor(lhs, rhs) => Ok(op!("xor", lhs, rhs)),
            Expr::Div(lhs, rhs) => {
                if let Some(Type::Float) = ctx.typed.get(&hash!(self)) {
                    return Ok(op!("div", lhs, rhs));
                }
                Ok(format!(
                    "{}\tpush rax\n{}\tmov rsi, rax\n\tpop rax\n\tcqo\n\tidiv rsi\n",
                    lhs.emit(ctx)?,
                    rhs.emit(ctx)?,
                ))
            }
            Expr::Mod(lhs, rhs) => {
                let div = Expr::Div(lhs.clone(), rhs.clone());
                Ok(div.emit(ctx)? + "\tmov rax, rdx\n")
            }
        }
    }
}
