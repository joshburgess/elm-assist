use crate::project::Project;

/// Sort all import declarations alphabetically in every module.
pub fn sort_imports(project: &mut Project) -> usize {
    let mut changes = 0;

    for file in &mut project.files {
        let before: Vec<String> = file
            .module
            .imports
            .iter()
            .map(|i| i.value.module_name.value.join("."))
            .collect();

        file.module
            .imports
            .sort_by(|a, b| a.value.module_name.value.cmp(&b.value.module_name.value));

        let after: Vec<String> = file
            .module
            .imports
            .iter()
            .map(|i| i.value.module_name.value.join("."))
            .collect();

        if before != after {
            changes += 1;
        }
    }

    changes
}
