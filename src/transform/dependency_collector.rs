use std::collections::HashSet;

use swc_atoms::JsWord;
use swc_common::SyntaxContext;
use swc_ecmascript::ast;
use swc_ecmascript::utils::ident::IdentLike;
use swc_ecmascript::visit::{Fold, FoldWith};

use super::utils::*;

/// This pass collects dependencies in a module and compiles references as needed to work with Parcel's JSRuntime.
pub fn dependency_collector<'a>(
    items: &'a mut HashSet<String>,
    decls: &'a HashSet<(JsWord, SyntaxContext)>,
) -> impl Fold + 'a {
    DependencyCollector { items, decls }
}

pub struct DependencyCollector<'a> {
    items: &'a mut HashSet<String>,
    decls: &'a HashSet<(JsWord, SyntaxContext)>,
}

impl<'a> DependencyCollector<'a> {
    fn add_dependency(&mut self, specifier: JsWord) {
        self.items.insert(specifier.to_string());
    }
}

impl<'a> Fold for DependencyCollector<'a> {
    fn fold_call_expr(&mut self, node: ast::CallExpr) -> ast::CallExpr {
        use ast::{Callee::*, Expr::*};

        let call_expr = match node.callee.clone() {
            Expr(boxed) => boxed,
            // Super and import
            _ => return node,
        };

        match &*call_expr {
            Ident(ident) => {
                // Bail if defined in scope
                if self.decls.contains(&ident.to_id()) {
                    return node.fold_children_with(self);
                }

                if ident.sym.to_string().as_str() != "require" {
                    return node.fold_children_with(self);
                }
            }
            _ => return node.fold_children_with(self),
        };

        if let Some(arg) = node.args.get(0) {
            if let Some((specifier, _)) = match_str(&*arg.expr) {
                self.add_dependency(specifier);
            }
        };

        node.fold_children_with(self)
    }
}
