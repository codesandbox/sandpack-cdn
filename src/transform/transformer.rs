use crate::app_error::ServerError;
use std::collections::{HashMap, HashSet};

use crate::transform::decl_collector::collect_decls;
use crate::transform::dependency_collector::dependency_collector;
use crate::transform::env_replacer::EnvReplacer;
use swc_atoms::{js_word, JsWord};
use swc_common::comments::SingleThreadedComments;
use swc_common::{chain, sync::Lrc, FileName, Globals, Mark, SourceMap};
use swc_ecma_preset_env::{preset_env, Mode::Entry, Targets, Version, Versions};
use swc_ecmascript::ast::Module;
use swc_ecmascript::codegen::text_writer::JsWriter;
use swc_ecmascript::parser::lexer::Lexer;
use swc_ecmascript::parser::{EsConfig, Parser, StringInput, Syntax};
use swc_ecmascript::transforms::modules::common_js::common_js;
use swc_ecmascript::transforms::modules::common_js::Config as CommonJSConfig;
use swc_ecmascript::transforms::resolver::resolver_with_mark;
use swc_ecmascript::transforms::Assumptions;
use swc_ecmascript::transforms::{
    compat::reserved_words::reserved_words, fixer, helpers, hygiene,
    optimization::simplify::dead_branch_remover, optimization::simplify::expr_simplifier,
    proposals::decorators,
};
use swc_ecmascript::visit::FoldWith;

#[derive(Debug)]
pub struct TransformedFile {
    pub content: String,
    pub dependencies: HashSet<String>,
}

fn parse(
    code: &str,
    source_map: &Lrc<SourceMap>,
) -> Result<(Module, SingleThreadedComments), ServerError> {
    // Attempt to convert the path to be relative to the project root.
    // If outside the project root, use an absolute path so that if the project root moves the path still works.
    let source_file = source_map.new_source_file(FileName::Anon, code.into());

    let comments = SingleThreadedComments::default();
    let syntax = Syntax::Es(EsConfig {
        jsx: false,
        export_default_from: true,
        decorators: true,
        ..Default::default()
    });

    let lexer = Lexer::new(
        syntax,
        Default::default(),
        StringInput::from(&*source_file),
        Some(&comments),
    );

    let mut parser = Parser::new_from(lexer);
    match parser.parse_module() {
        Err(err) => Err(ServerError::SWCParseError {
            message: format!("{:?}", err),
        }),
        Ok(module) => Ok((module, comments)),
    }
}

fn get_versions() -> Versions {
    // based on `npx browserslist ">1%, not ie 11, not op_mini all"`
    let mut versions = Versions::default();
    versions.chrome = Some(Version {
        major: 93,
        minor: 0,
        patch: 0,
    });
    versions.edge = Some(Version {
        major: 94,
        minor: 0,
        patch: 0,
    });
    versions.firefox = Some(Version {
        major: 93,
        minor: 0,
        patch: 0,
    });
    versions.ios = Some(Version {
        major: 14,
        minor: 0,
        patch: 0,
    });
    versions.safari = Some(Version {
        major: 14,
        minor: 1,
        patch: 0,
    });
    versions
}

pub fn transform_file(filename: &str, code: &str) -> Result<TransformedFile, ServerError> {
    println!("Transforming file: {}", filename);

    // Error early if filename does not end in js: js, cjs, mjs, ...
    let ext = &filename[filename.len() - 2..];
    if ext.ne("js") {
        return Err(ServerError::SWCParseError {
            message: format!("File {} is not JavaScript", filename),
        });
    }

    let source_map = Lrc::new(SourceMap::default());
    let (mut module, comments) = parse(code, &source_map)?;

    swc_common::GLOBALS.set(&Globals::new(), || {
        helpers::HELPERS.set(
            &helpers::Helpers::new(/* external helpers from @swc/helpers */ true),
            || {
                let global_mark = Mark::fresh(Mark::root());
                module = {
                    let mut passes = chain!(
                        // Decorators can use type information, so must run before the TypeScript pass.
                        decorators::decorators(decorators::Config {
                            legacy: true,
                            // Always disabled for now, SWC's implementation doesn't match TSC.
                            emit_metadata: false
                        }),
                        resolver_with_mark(global_mark),
                    );

                    module.fold_with(&mut passes)
                };

                let mut preset_env_config = swc_ecma_preset_env::Config {
                    dynamic_import: true,
                    ..Default::default()
                };

                let versions = get_versions();
                preset_env_config.targets = Some(Targets::Versions(versions));
                preset_env_config.shipped_proposals = true;
                preset_env_config.mode = Some(Entry);
                preset_env_config.bugfixes = true;

                let mut env: HashMap<JsWord, JsWord> = HashMap::new();
                env.insert(js_word!("NODE_ENV"), JsWord::from("development"));

                let mut decls = collect_decls(&module);

                // dead code elimination and env inlining
                let module = {
                    let mut passes = chain!(
                        // Inline process.env and process.browser
                        EnvReplacer {
                            env: &env,
                            is_browser: true,
                            decls: &decls,
                        },
                        // Simplify expressions and remove dead branches so that we
                        // don't include dependencies inside conditionals that are always false.
                        expr_simplifier(Default::default()),
                        dead_branch_remover(),
                    );

                    module.fold_with(&mut passes)
                };

                // Run preset_env
                let module = {
                    let mut passes = chain!(
                        // Transpile new syntax to older syntax if needed
                        preset_env(
                            global_mark,
                            Some(&comments),
                            preset_env_config,
                            Assumptions::all()
                        ),
                        // Inject SWC helpers if needed.
                        helpers::inject_helpers(),
                    );

                    module.fold_with(&mut passes)
                };

                // convert down to commonjs
                let module = {
                    let commonjs_config = CommonJSConfig::default();

                    let mut passes = chain!(
                        resolver_with_mark(global_mark),
                        common_js(global_mark, commonjs_config, None)
                    );

                    module.fold_with(&mut passes)
                };

                // Collect dependencies - ALWAYS RUN THIS LAST
                decls = collect_decls(&module);
                let mut dependencies: HashSet<String> = HashSet::new();
                let module = module.fold_with(&mut dependency_collector(&mut dependencies, &decls));

                let program = {
                    let mut passes = chain!(reserved_words(), hygiene(), fixer(Some(&comments)),);
                    module.fold_with(&mut passes)
                };

                // Remove sourcemap comment
                {
                    let (mut _leading_comments, mut trailing_comments) = comments.borrow_all_mut();
                    for (_key, value) in trailing_comments.iter_mut() {
                        if let Some(index) = value
                            .iter()
                            .position(|comment| comment.text.starts_with("# sourceMappingURL"))
                        {
                            value.remove(index);
                        }
                    }
                }

                // Print code...
                let mut buf = vec![];
                let writer = Box::new(JsWriter::new(source_map.clone(), "\n", &mut buf, None));
                let emitter_config = swc_ecmascript::codegen::Config { minify: true };
                let mut emitter = swc_ecmascript::codegen::Emitter {
                    cfg: emitter_config,
                    comments: Some(&comments),
                    cm: source_map,
                    wr: writer,
                };
                emitter.emit_module(&program)?;

                let output = String::from(std::str::from_utf8(&buf).unwrap_or(""));

                Ok(TransformedFile {
                    content: output,
                    dependencies,
                })
            },
        )
    })
}

#[cfg(test)]
mod test {
    use crate::transform::transformer::transform_file;

    #[test]
    fn inlines_env_variables() {
        assert_eq!(
            transform_file("index.js", "module.exports = process.env.NODE_ENV;")
                .unwrap()
                .content,
            String::from("\"use strict\";module.exports=\"development\";")
        );
    }

    #[test]
    fn remove_sourcemap_comment() {
        // TODO: Allow inline sourcemaps?
        assert_eq!(
            transform_file(
                "index.js",
                "module.exports = \"hello world\";\n//other-comment\n//# sourceMappingURL=index.js.map"
            )
            .unwrap()
            .content,
            String::from("\"use strict\";module.exports=\"hello world\"; //other-comment\n")
        );
    }
}
