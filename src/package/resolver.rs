use glob::Pattern;
use std::collections::{HashMap, HashSet, LinkedList};

pub fn make_mod_specifier_absolute(cwd: &str, mod_specifier: &str) -> String {
    let mut full_path = String::from(cwd);
    full_path.push_str("/");
    full_path.push_str(mod_specifier);
    let curr_path_parts = full_path.split("/");

    let mut result_parts: Vec<&str> = Vec::new();
    for part in curr_path_parts {
        if part == "." || part == "" {
            continue;
        } else if part == ".." {
            result_parts.pop();
        } else {
            result_parts.push(part);
        }
    }

    return result_parts.join("/");
}

pub fn file_path_to_dirname(file_path: &str) -> String {
    return make_mod_specifier_absolute(file_path, "..");
}

pub fn extract_file_extension(file_path: &str) -> Option<&str> {
    if let Some(ext_index) = file_path.rfind('.') {
        let ext_name = &file_path[ext_index..];
        if ext_name == "" || ext_name == "." || ext_name.contains("/") {
            return None;
        } else {
            return Some(ext_name);
        }
    }
    return None;
}

pub fn collect_files(
    abs_file_pattern: &str,
    files_map: &HashMap<String, u64>,
    optional_curr_ext: Option<&str>,
) -> Vec<String> {
    if abs_file_pattern.contains("*") {
        // if we can't turn this into a pattern we assume it's an invalid pattern and see it as a simple module specifier
        if let Ok(glob_pattern) = Pattern::new(abs_file_pattern) {
            let extensions =
                HashSet::from([".js", ".mjs", ".cjs", ".css", ".sass", ".scss", ".less"]);
            let mut result: Vec<String> = vec![];
            for file_path in files_map.keys() {
                if let Some(file_ext) = extract_file_extension(file_path) {
                    if extensions.contains(file_ext) && glob_pattern.matches(file_path) {
                        result.push(file_path.to_owned());
                    }
                }
            }
            return result;
        }
    }

    // return early if file simply exists
    if files_map.contains_key(abs_file_pattern) {
        return vec![String::from(abs_file_pattern)];
    }

    let mut extensions =
        LinkedList::from([".js", ".mjs", ".cjs", ".css", ".sass", ".scss", ".less"]);
    // the current extension is preferred
    if let Some(curr_ext) = optional_curr_ext {
        extensions.push_front(curr_ext);
    }

    // check all extensions
    for ext in extensions.iter() {
        let mut concatenated_str = String::from(abs_file_pattern);
        concatenated_str.push_str(ext);

        if files_map.contains_key(concatenated_str.as_str()) {
            return vec![String::from(concatenated_str)];
        }
    }

    // check all /index.ext variants...
    for ext in extensions.iter() {
        let mut concatenated_str = String::from(abs_file_pattern);
        concatenated_str.push_str("/index");
        concatenated_str.push_str(ext);

        if files_map.contains_key(concatenated_str.as_str()) {
            return vec![String::from(concatenated_str)];
        }
    }

    // fallback to empty vector, in this case we won't do anything
    return vec![];
}

#[cfg(test)]
mod test {
    use crate::package::resolver::{
        collect_files, extract_file_extension, file_path_to_dirname, make_mod_specifier_absolute,
    };
    use std::collections::HashMap;

    #[test]
    fn basic_abs_mod_specifiers() {
        assert_eq!(
            make_mod_specifier_absolute(".", "./dist/a.js"),
            String::from("dist/a.js")
        );
        assert_eq!(
            make_mod_specifier_absolute("deeply/nested/directory/", "../dist/*"),
            String::from("deeply/nested/dist/*")
        );
    }

    #[test]
    fn get_dirname() {
        assert_eq!(file_path_to_dirname("./dist/a.js"), String::from("dist"));
        assert_eq!(
            file_path_to_dirname("deeply/nested/directory/abc.js"),
            String::from("deeply/nested/directory")
        );
    }

    #[test]
    fn get_extname() {
        assert_eq!(extract_file_extension("something.js"), Some(".js"));
        assert_eq!(extract_file_extension("."), None);
        assert_eq!(
            extract_file_extension("./test/.something/test.js"),
            Some(".js")
        );
        assert_eq!(extract_file_extension("./test/.something/test"), None);
    }

    #[test]
    fn collect_filenames() {
        let mut files_map: HashMap<String, u64> = HashMap::new();
        files_map.insert(String::from("deeply/nested/index.js"), 123);
        files_map.insert(String::from("index.js"), 123);
        files_map.insert(String::from("component/Button.mjs"), 123);
        files_map.insert(String::from("component/Link.cjs"), 123);
        files_map.insert(String::from("component/Link.js"), 123);
        files_map.insert(String::from("component/Link/index.js"), 123);
        assert_eq!(
            collect_files("deeply/nested", &files_map, None),
            vec!["deeply/nested/index.js"]
        );
        assert_eq!(
            collect_files("component/Link", &files_map, Some(".cjs")),
            vec!["component/Link.cjs"]
        );
        assert_eq!(
            collect_files("index", &files_map, Some(".mjs")),
            vec!["index.js"]
        );
        assert_eq!(
            collect_files("component/Link", &files_map, Some(".mjs")),
            vec!["component/Link.js"]
        );
    }

    #[test]
    fn collect_glob_filenames() {
        let mut files_map: HashMap<String, u64> = HashMap::new();
        files_map.insert(String::from("deeply/nested/index.js"), 123);
        files_map.insert(String::from("index.js"), 123);
        files_map.insert(String::from("component/Button.mjs"), 123);
        files_map.insert(String::from("component/Link.cjs"), 123);
        files_map.insert(String::from("component/Link.js"), 123);
        files_map.insert(String::from("component/Link/index.js"), 123);
        assert_eq!(
            collect_files("deeply/*", &files_map, None),
            vec!["deeply/nested/index.js"]
        );
        assert_eq!(
            collect_files("component/*", &files_map, Some(".cjs")).sort(),
            vec![
                "component/Button.mjs",
                "component/Link.cjs",
                "component/Link.js",
                "component/Link/index.js"
            ]
            .sort()
        );
        let empty_vec: Vec<String> = Vec::new();
        assert_eq!(
            collect_files("something-non-existing/*", &files_map, Some(".cjs")),
            empty_vec
        );
    }
}
