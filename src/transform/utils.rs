use std::collections::HashSet;
use swc_atoms::JsWord;
use swc_common::{Span, SyntaxContext};
use swc_ecmascript::ast::{self, MemberProp};

pub fn match_member_expr(
    expr: &ast::MemberExpr,
    idents: Vec<&str>,
    decls: &HashSet<(JsWord, SyntaxContext)>,
) -> bool {
    use ast::{Expr::*, Ident};

    let mut member = expr;
    let mut idents = idents;
    while idents.len() > 1 {
        let expected = idents.pop().unwrap();
        let prop = match &member.prop {
            MemberProp::Ident(Ident { ref sym, .. }) => sym,
            _ => return false,
        };

        if prop != expected {
            return false;
        }

        match &*member.obj {
            Member(m) => member = m,
            Ident(Ident { ref sym, span, .. }) => {
                return idents.len() == 1
                    && sym == idents.pop().unwrap()
                    && !decls.contains(&(sym.clone(), span.ctxt()));
            }
            _ => return false,
        }
    }

    false
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
