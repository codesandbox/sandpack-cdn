#[cfg(test)]
mod tmp {
    use crate::transform::transformer::transform_file;

    #[test]
    fn index_js_panics() {
        let code = "'use strict';\nif (process.env.NODE_ENV === 'production') {\n  module.exports = require('./cjs/react.production.js');\n} else {\n  module.exports = require('./cjs/react.development.js');\n}\n";
        let _ = transform_file("index.js", code).unwrap();
    }
}
