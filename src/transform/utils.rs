use std::collections::HashSet;

use swc_atoms::{JsWord, js_word};
use swc_common::{Mark, Span, SyntaxContext, DUMMY_SP};
use swc_ecmascript::ast;

pub fn match_member_expr(
    expr: &ast::MemberExpr,
    idents: Vec<&str>,
    decls: &HashSet<(JsWord, SyntaxContext)>,
) -> bool {
    use ast::{Expr::*, ExprOrSuper::*, Ident, Lit, Str};

    let mut member = expr;
    let mut idents = idents;
    while idents.len() > 1 {
        let expected = idents.pop().unwrap();
        let prop = match &*member.prop {
            Lit(Lit::Str(Str { value: ref sym, .. })) => sym,
            Ident(Ident { ref sym, .. }) => {
                if member.computed {
                    return false;
                }

                sym
            }
            _ => return false,
        };

        if prop != expected {
            return false;
        }

        match &member.obj {
            Expr(expr) => match &**expr {
                Member(m) => member = m,
                Ident(Ident { ref sym, span, .. }) => {
                    return idents.len() == 1
                        && sym == idents.pop().unwrap()
                        && !decls.contains(&(sym.clone(), span.ctxt()));
                }
                _ => return false,
            },
            _ => return false,
        }
    }

    false
}

pub fn create_require(specifier: swc_atoms::JsWord) -> ast::CallExpr {
    let mut normalized_specifier = specifier;
    if normalized_specifier.starts_with("node:") {
        normalized_specifier = normalized_specifier.replace("node:", "").into();
    }

    ast::CallExpr {
        callee: ast::ExprOrSuper::Expr(Box::new(ast::Expr::Ident(ast::Ident::new(
            "require".into(),
            DUMMY_SP,
        )))),
        args: vec![ast::ExprOrSpread {
            expr: Box::new(ast::Expr::Lit(ast::Lit::Str(ast::Str {
                span: DUMMY_SP,
                value: normalized_specifier,
                has_escape: false,
                kind: ast::StrKind::Synthesized,
            }))),
            spread: None,
        }],
        span: DUMMY_SP,
        type_args: None,
    }
}

fn is_marked(span: Span, mark: Mark) -> bool {
    let mut ctxt = span.ctxt();

    loop {
        let m = ctxt.remove_mark();
        if m == Mark::root() {
            return false;
        }

        if m == mark {
            return true;
        }
    }
}

pub fn match_str(node: &ast::Expr) -> Option<(JsWord, Span)> {
    use ast::*;

    match node {
        // "string" or 'string'
        Expr::Lit(Lit::Str(s)) => Some((s.value.clone(), s.span)),
        // `string`
        Expr::Tpl(tpl) if tpl.quasis.len() == 1 && tpl.exprs.is_empty() => {
            Some((tpl.quasis[0].raw.value.clone(), tpl.span))
        }
        _ => None,
    }
}

pub fn match_str_or_ident(node: &ast::Expr) -> Option<(JsWord, Span)> {
    use ast::*;

    if let Expr::Ident(id) = node {
        return Some((id.sym.clone(), id.span));
    }

    match_str(node)
}

pub fn match_require(
    node: &ast::Expr,
    decls: &HashSet<(JsWord, SyntaxContext)>,
    ignore_mark: Mark,
) -> Option<JsWord> {
    use ast::*;

    match node {
        Expr::Call(call) => match &call.callee {
            ExprOrSuper::Expr(expr) => match &**expr {
                Expr::Ident(ident) => {
                    if ident.sym == js_word!("require")
                        && !decls.contains(&(ident.sym.clone(), ident.span.ctxt))
                        && !is_marked(ident.span, ignore_mark)
                    {
                        if let Some(arg) = call.args.get(0) {
                            return match_str(&*arg.expr).map(|(name, _)| name);
                        }
                    }

                    None
                }
                Expr::Member(member) => {
                    if match_member_expr(member, vec!["module", "require"], decls) {
                        if let Some(arg) = call.args.get(0) {
                            return match_str(&*arg.expr).map(|(name, _)| name);
                        }
                    }

                    None
                }
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

pub fn match_import(node: &ast::Expr, ignore_mark: Mark) -> Option<JsWord> {
    use ast::*;

    match node {
        Expr::Call(call) => match &call.callee {
            ExprOrSuper::Expr(expr) => match &**expr {
                Expr::Ident(ident) => {
                    if ident.sym == js_word!("import") && !is_marked(ident.span, ignore_mark) {
                        if let Some(arg) = call.args.get(0) {
                            return match_str(&*arg.expr).map(|(name, _)| name);
                        }
                    }

                    None
                }
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

#[macro_export]
macro_rules! fold_member_expr_skip_prop {
    () => {
        fn fold_member_expr(
            &mut self,
            mut node: swc_ecmascript::ast::MemberExpr,
        ) -> swc_ecmascript::ast::MemberExpr {
            node.obj = node.obj.fold_with(self);

            if node.computed {
                node.prop = node.prop.fold_with(self);
            }

            node
        }
    };
}

#[macro_export]
macro_rules! id {
    ($ident: expr) => {
        ($ident.sym.clone(), $ident.span.ctxt)
    };
}

pub type IdentId = (JsWord, SyntaxContext);
