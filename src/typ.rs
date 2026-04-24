use crate::*;

impl Define {
    pub fn infer(&self, ctx: &mut Context) -> Result<Type, String> {
        match self {
            Define::Function(name, args, body) => {
                ctx.current = name.to_string();
                ctx.local().var = args.clone();

                for (name, arg) in args.clone() {
                    ctx.local().var.insert(name, arg);
                }

                let return_type = body.infer(ctx)?;
                let signature = Type::Function(
                    Box::new(return_type.clone()),
                    Some(args.values().cloned().collect::<Vec<Type>>()),
                );
                ctx.global.lib.insert(name.clone(), signature);
                Ok(return_type)
            }
            Define::Class(name, layout) => {
                ctx.global.vtable.insert(name.clone(), layout.clone());
                Ok(Type::None)
            }
        }
    }
}

impl Expr {
    pub fn infer(&self, ctx: &mut Context) -> Result<Type, String> {
        macro_rules! typing {
            ($typ: expr) => {{
                ctx.typed.insert(hash!(self), $typ);
                Ok::<Type, String>($typ)
            }};
        }
        macro_rules! op {
            ($typ: pat, $lhs: expr, $rhs: expr, $genre: literal) => {{
                let lt = $lhs.infer(ctx)?;
                let rt = $rhs.infer(ctx)?;
                if lt == rt {
                    let typ = lt;
                    #[allow(warnings)]
                    if let $typ = typ {
                        typing!(typ.clone())
                    } else {
                        Err(format!("{typ:?}型は{}の項に出来ません", $genre))
                    }
                } else {
                    Err(format!("左辺と右辺の型が異なります: {lt:?} != {rt:?}"))
                }
            }};
        }
        macro_rules! access {
            ($object: expr, $property: expr) => {{
                let Type::Class(class_name, arg) = $object.infer(ctx)? else {
                    return Err(format!("オブジェクトでないとメンバ変数を読み込めません"));
                };
                let Some(layout) = ctx.global.vtable.get(&class_name) else {
                    return Err(format!("取得するクラスが見つかりません: {class_name}"));
                };
                let Some(typ) = layout.get($property) else {
                    return Err(format!("{class_name}型に無いメンバ変数: {}", $property));
                };
                (typ.clone(), arg)
            }};
        }

        match self {
            Expr::If(cond, then, els) => {
                if cond.infer(ctx)? != Type::Bool {
                    return Err(format!("条件式にはBool型が必要です"));
                }
                if let Some(els) = els {
                    op!(_, then, els, "条件分岐")
                } else {
                    then.infer(ctx)?;
                    Ok(Type::None)
                }
            }
            Expr::While(cond, body) => {
                if cond.infer(ctx)? != Type::Bool {
                    return Err(format!("条件式にはBool型が必要です"));
                }
                body.infer(ctx)
            }
            Expr::Block(lines) => {
                let mut return_value = Type::None;
                for line in lines {
                    return_value = line.infer(ctx)?;
                }
                typing!(return_value.clone())
            }
            Expr::Let(name, value) => match &**name {
                Expr::Variable(name) => {
                    let typ = value.infer(ctx)?;
                    let env = &mut ctx.local().var;
                    if let Some(old_typ) = env.get(name) {
                        if typ != *old_typ {
                            return Err(format!(
                                "変数の型と代入する値が異なります: {old_typ:?} != {typ:?}"
                            ));
                        }
                    } else {
                        env.insert(name.clone(), typ.clone());
                    }
                    typing!(typ.clone())
                }
                Expr::Access(object, property) => {
                    let val = value.infer(ctx)?;
                    let (mut typ, arg) = access!(object, property);
                    if let Type::Class(class_name, _) = &typ {
                        if *class_name == Name::new("T")? {
                            if let Some(arg) = arg {
                                typ = *arg.clone();
                            } else {
                                return Err(format!("型引数が必要です"));
                            }
                        }
                    }
                    if typ.clone() != val {
                        return Err(format!(
                            "メンバ変数の型と代入する値が異なります: {typ:?} != {val:?}"
                        ));
                    }
                    typing!(typ.clone())
                }
                _ => Err(format!("代入対象が間違ってます")),
            },
            Expr::Variable(name) => {
                if let Some(typ) = ctx.local().var.get(name).cloned() {
                    typing!(typ.clone())
                } else {
                    if let Some(typ) = ctx.global.lib.get(name) {
                        typing!(typ.clone())
                    } else {
                        Err(format!("変数が未だ宣言されてません: {name}"))
                    }
                }
            }
            Expr::Call(calee, args) => {
                let typ = calee.infer(ctx)?;
                if let Type::Function(ret, params) = typ {
                    if let Some(params) = params {
                        if params.len() != args.len() {
                            return Err(format!("引数に過不足があります"));
                        }
                        for (param, arg) in params.iter().zip(args) {
                            let arg = arg.infer(ctx)?;
                            if arg != *param {
                                return Err(format!(
                                    "関数の期待する仮引数と渡された実引数の型が異なります: {param:?} != {arg:?}"
                                ));
                            }
                        }
                    } else {
                        for arg in args {
                            arg.infer(ctx)?;
                        }
                    }
                    typing!(*ret.clone())
                } else {
                    Err(format!("関数以外は呼び出し出来ません: {typ:?}"))
                }
            }
            Expr::New(typ) => {
                if let Type::Class(class_name, _) = typ {
                    if let Some(layout) = ctx.global.vtable.get(class_name) {
                        initializer!(layout).infer(ctx)?;
                        typing!(typ.clone())
                    } else {
                        Err(format!("構築するクラスが見つかりません: {class_name}"))
                    }
                } else {
                    Err(format!("基本型は構築できません"))
                }
            }
            Expr::Access(object, property) => {
                let (typ, arg) = access!(object, property);
                if let Type::Class(class_name, _) = &typ {
                    if *class_name == Name::new("T")? {
                        return if let Some(arg) = arg {
                            typing!(*arg.clone())
                        } else {
                            Err(format!("型引数が必要です"))
                        };
                    }
                }
                typing!(typ.clone())
            }
            Expr::Nullcheck(expr) => {
                if !matches!(expr.infer(ctx)?, Type::Class(_, _)) {
                    return Err(format!("基本型ではNullチェック出来ません"));
                }
                typing!(Type::Bool)
            }
            Expr::Transmute(expr, typ) => {
                expr.infer(ctx)?;
                typing!(typ.clone())
            }
            Expr::Read(_, typ, addr) => {
                addr.infer(ctx)?;
                typing!(typ.clone())
            }
            Expr::Write(_, value, addr) => {
                addr.infer(ctx)?;
                let value = value.clone().infer(ctx)?;
                typing!(value.clone())
            }
            Expr::Integer(_) => typing!(Type::Integer),
            Expr::Float(_) => typing!(Type::Float),
            Expr::String(_) => typing!(Type::String),
            Expr::Bool(_) => typing!(Type::Bool),
            Expr::Add(lhs, rhs) => op!(Type::Integer | Type::Float, lhs, rhs, "足す演算"),
            Expr::Sub(lhs, rhs) => op!(Type::Integer | Type::Float, lhs, rhs, "引く演算"),
            Expr::Mul(lhs, rhs) => op!(Type::Integer | Type::Float, lhs, rhs, "掛ける演算"),
            Expr::Div(lhs, rhs) => op!(Type::Integer | Type::Float, lhs, rhs, "割る演算"),
            Expr::Mod(lhs, rhs) => op!(Type::Integer, lhs, rhs, "余り演算"),
            Expr::Eql(lhs, rhs) => op!(Type::Integer, lhs, rhs, "等価比較"),
            Expr::NotEq(lhs, rhs) => op!(Type::Integer, lhs, rhs, "非等価比較"),
            Expr::Gt(lhs, rhs) => op!(Type::Integer, lhs, rhs, "大なり比較"),
            Expr::Lt(lhs, rhs) => op!(Type::Integer, lhs, rhs, "小なり比較"),
            Expr::GtEq(lhs, rhs) => op!(Type::Integer, lhs, rhs, "大なり等価比較"),
            Expr::LtEq(lhs, rhs) => op!(Type::Integer, lhs, rhs, "小なり等価比較"),
            Expr::And(lhs, rhs) => op!(Type::Bool, lhs, rhs, "論理積"),
            Expr::Or(lhs, rhs) => op!(Type::Bool, lhs, rhs, "論理和"),
            Expr::Xor(lhs, rhs) => op!(Type::Bool, lhs, rhs, "排他的論理和"),
        }
    }
}

impl Context {
    pub fn local(&mut self) -> &mut Function {
        let key = Name::new(&self.current.clone()).unwrap();
        self.local.get_mut(&key).unwrap()
    }
}
