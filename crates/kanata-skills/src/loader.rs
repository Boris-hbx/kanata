//! YAML skill loader — loads skill definitions from YAML files.

use std::path::Path;

use kanata_types::skill::SkillDefinition;

/// Loads a single skill definition from a YAML file.
///
/// Expected YAML format:
/// ```yaml
/// name: commit
/// description: Generate a commit message from git diff
/// prompt_template: |
///   Analyze the following git diff and generate a commit message...
/// ```
///
/// # Errors
///
/// Returns an error if the file cannot be read or parsed as valid YAML.
pub fn load_skill_from_yaml(path: &Path) -> Result<SkillDefinition, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    serde_yaml::from_str(&content)
        .map_err(|e| format!("Failed to parse YAML {}: {e}", path.display()))
}

/// Loads all `.skill.yaml` files from a directory.
///
/// # Errors
///
/// Returns an error if the directory cannot be read or any skill file is invalid.
pub fn load_skills_from_dir(dir: &Path) -> Result<Vec<SkillDefinition>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut skills = Vec::new();
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("Failed to read dir {}: {e}", dir.display()))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
        let path = entry.path();
        if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".skill.yaml")) {
            let skill = load_skill_from_yaml(&path)?;
            tracing::info!(name = %skill.name, "Loaded skill");
            skills.push(skill);
        }
    }

    Ok(skills)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_skill_from_yaml() {
        let dir = std::env::temp_dir().join("kanata_test_skill_loader");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.skill.yaml");
        std::fs::write(
            &file,
            r#"
name: test-skill
description: A test skill
prompt_template: "Do something: {{input}}"
"#,
        )
        .unwrap();

        let skill = load_skill_from_yaml(&file).unwrap();
        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.description, "A test skill");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_skills_from_dir() {
        let dir = std::env::temp_dir().join("kanata_test_skill_dir");
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(
            dir.join("a.skill.yaml"),
            "name: a\ndescription: Skill A\nprompt_template: A\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("b.skill.yaml"),
            "name: b\ndescription: Skill B\nprompt_template: B\n",
        )
        .unwrap();
        // This file should be ignored (wrong extension).
        std::fs::write(dir.join("readme.txt"), "not a skill").unwrap();

        let skills = load_skills_from_dir(&dir).unwrap();
        assert_eq!(skills.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_skills_from_nonexistent_dir() {
        let skills = load_skills_from_dir(Path::new("/nonexistent/dir")).unwrap();
        assert!(skills.is_empty());
    }
}
