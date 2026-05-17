use crate::project::AgentContext;
use std::path::Path;

/// Parse agent/project management files and READMEs for structured context
pub fn parse_agent_files(project_path: &Path) -> Option<AgentContext> {
    let mut context = AgentContext::default();
    let mut found_readme = false;

    // --- README / docs parsing (project description) ---
    let readme_candidates = [
        "README.md", "README", "readme.md", "Readme.md",
    ];
    for filename in &readme_candidates {
        let file_path = project_path.join(filename);
        if file_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                if let Some(desc) = extract_readme_description(&content) {
                    context.description = Some(desc);
                    found_readme = true;
                    break;
                }
            }
        }
    }

    // If no README, try extracting a description from brief.md or summary.md
    if !found_readme {
        for filename in &["brief.md", "summary.md", "project-summary.md"] {
            let file_path = project_path.join(filename);
            if file_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    if let Some(desc) = extract_first_paragraph(&content) {
                        context.description = Some(desc);
                        break;
                    }
                }
            }
        }
    }

    // --- Agent / task tracking files ---
    let agent_files = [
        "CLAUDE.md", "AGENTS.md", "PLAN.md", "ROADMAP.md", "TODO.md",
    ];

    for filename in &agent_files {
        let file_path = project_path.join(filename);
        if file_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                parse_agent_content(&content, &mut context);
            }
        }
    }

    // Only return if we found something useful
    if context.description.is_some()
        || context.current_phase.is_some()
        || context.current_task.is_some()
        || !context.next_steps.is_empty()
    {
        Some(context)
    } else {
        None
    }
}

/// Extract a one-line description from a README:
/// find the first H1 heading, then take the first non-empty paragraph after it.
fn extract_readme_description(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut in_header = false;

    for line in lines.iter() {
        let trimmed = line.trim();

        // Skip badges and image-only lines
        if trimmed.starts_with("[!") || trimmed.starts_with("<img") || trimmed.contains("![") {
            continue;
        }

        // Find the first H1 (# Title)
        if trimmed.starts_with("# ") && !trimmed.starts_with("##") {
            in_header = true;
            continue;
        }

        // After finding the H1, collect consecutive non-empty, non-heading lines
        if in_header {
            if trimmed.is_empty() {
                continue; // skip blank lines between header and content
            }
            if trimmed.starts_with('#') {
                break; // hit another heading, stop
            }
            // Skip badges, shilds, image embeds, action links
            if trimmed.starts_with("[![") || trimmed.starts_with("<a ") || trimmed.starts_with("```") {
                continue;
            }
            // Clean up the paragraph: strip leading "> " (blockquotes), trim
            let clean = trimmed.trim_start_matches("> ").trim();
            if clean.len() > 20 {
                return Some(truncate_description(clean));
            }
        }
    }

    // Fallback: just take the first meaningful line
    extract_first_paragraph(content)
}

/// Extract the first non-trivial paragraph from any markdown file.
fn extract_first_paragraph(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("![") {
            continue;
        }
        if trimmed.starts_with("[![") || trimmed.starts_with("<a ") || trimmed.starts_with("```") {
            continue;
        }
        let clean = trimmed.trim_start_matches("> ").trim();
        if clean.len() > 20 {
            return Some(truncate_description(clean));
        }
    }
    None
}

fn truncate_description(s: &str) -> String {
    if s.len() > 200 {
        let mut truncated = String::with_capacity(203);
        truncated.push_str(&s[..197]);
        truncated.push_str("...");
        truncated
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_readme_description_simple() {
        let content = "# My Project\n\nThis is a cool project that does things.\n\nMore details here.";
        let desc = extract_readme_description(content);
        assert_eq!(desc, Some("This is a cool project that does things.".to_string()));
    }

    #[test]
    fn test_extract_readme_description_with_badges() {
        let content = "# My Project\n\n[![CI](https://img.shields.io/badge/ci-passing.svg)](https://example.com)\n\nThis is the real description after badges.\n\nMore stuff.";
        let desc = extract_readme_description(content);
        assert_eq!(desc, Some("This is the real description after badges.".to_string()));
    }

    #[test]
    fn test_extract_readme_description_fallback() {
        let content = "Just a simple file\n\nWith some description here.\n\nNo H1 heading.";
        let desc = extract_readme_description(content);
        assert_eq!(desc, Some("With some description here.".to_string()));
    }

    #[test]
    fn test_extract_readme_description_too_short() {
        let content = "# Project\n\nShort.";
        let desc = extract_readme_description(content);
        assert!(desc.is_none() || desc.unwrap().len() <= 20);
    }

    #[test]
    fn test_truncate_description() {
        let short = "Hello, world!";
        assert_eq!(truncate_description(short), short);

        let long = "x".repeat(250);
        let truncated = truncate_description(&long);
        assert_eq!(truncated.len(), 200);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_extract_first_paragraph() {
        let content = "# Heading\n\nThis is the first real paragraph with enough text.\n\nAnother para.";
        let result = extract_first_paragraph(content);
        assert_eq!(result, Some("This is the first real paragraph with enough text.".to_string()));
    }

    #[test]
    fn test_parse_agent_content_checklist() {
        let content = "# Project Plan\n\n## Tasks\n- [ ] do something\n- [x] done task\n- [ ] another todo\n";
        let mut context = AgentContext::default();
        parse_agent_content(content, &mut context);

        assert_eq!(context.checklist_total, 3);
        assert_eq!(context.checklist_done, 1);
        assert_eq!(context.next_steps.len(), 2);
        assert_eq!(context.completed_items.len(), 1);
        assert!(context.next_steps.contains(&"do something".to_string()));
        assert!(context.completed_items.contains(&"done task".to_string()));
    }

    #[test]
    fn test_parse_agent_content_phase() {
        let content = "## Phase\n\nInitial Development\n\n## Task\n\nBuild the core module";
        let mut context = AgentContext::default();
        parse_agent_content(content, &mut context);

        assert_eq!(context.current_phase, Some("Initial Development".to_string()));
        assert_eq!(context.current_task, Some("Build the core module".to_string()));
    }

    #[test]
    fn test_parse_agent_content_blockers() {
        let content = "## Blocker: Waiting for API key\n\nSome other text\n\nblocked: dependency not released";
        let mut context = AgentContext::default();
        parse_agent_content(content, &mut context);

        assert!(!context.blockers.is_empty());
        assert!(context.blockers.iter().any(|b| b.contains("API key") || b.contains("dependency")));
    }

    #[test]
    fn test_parse_agent_content_next_steps_section() {
        let content = "# Plan\n\n## Next Steps\n- Step one\n- Step two\n- Step three\n\n## Other";
        let mut context = AgentContext::default();
        parse_agent_content(content, &mut context);

        assert!(context.next_steps.contains(&"Step one".to_string()));
        assert!(context.next_steps.contains(&"Step two".to_string()));
        assert!(context.next_steps.contains(&"Step three".to_string()));
    }

    #[test]
    fn test_parse_agent_content_decisions() {
        let content = "# Log\n\nDecision: Use Rust for backend\nChose: Axum over Actix";
        let mut context = AgentContext::default();
        parse_agent_content(content, &mut context);

        assert!(!context.recent_decisions.is_empty());
        assert!(context.recent_decisions.iter().any(|d| d.contains("Rust")));
    }

    #[test]
    fn test_extract_checklist_item() {
        assert_eq!(
            extract_checklist_item("- [ ] do the thing"),
            Some("do the thing".to_string())
        );
        assert_eq!(
            extract_checklist_item("- [x] completed item"),
            Some("completed item".to_string())
        );
        assert_eq!(
            extract_checklist_item("- [X] another done"),
            Some("another done".to_string())
        );
        assert_eq!(
            extract_checklist_item("* [ ] star bullet"),
            Some("star bullet".to_string())
        );
        assert_eq!(extract_checklist_item("not a checklist"), None);
    }

    #[test]
    fn test_parse_agent_files_empty_dir() {
        // Create a temp dir with no files
        let dir = std::env::temp_dir().join("panoptic-test-empty");
        let _ = std::fs::create_dir_all(&dir);
        let result = parse_agent_files(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_agent_files_with_readme() {
        let dir = std::env::temp_dir().join("panoptic-test-readme");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("README.md"), "# Test Project\n\nA description for testing.\n").unwrap();

        let result = parse_agent_files(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(result.is_some());
        assert_eq!(result.unwrap().description, Some("A description for testing.".to_string()));
    }

    #[test]
    fn test_parse_agent_files_with_brief() {
        let dir = std::env::temp_dir().join("panoptic-test-brief");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("brief.md"), "A brief description of the project.\n\nMore details.").unwrap();

        let result = parse_agent_files(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(result.is_some());
        assert_eq!(result.unwrap().description, Some("A brief description of the project.".to_string()));
    }

    #[test]
    fn test_parse_agent_files_full_context() {
        let dir = std::env::temp_dir().join("panoptic-test-full");
        let _ = std::fs::create_dir_all(&dir);

        std::fs::write(dir.join("README.md"), "# Full Project\n\nA full featured project.\n").unwrap();
        std::fs::write(dir.join("CLAUDE.md"), "# CLAUDE\n\n## Phase\n\nBeta\n\n## Tasks\n- [x] setup\n- [ ] build feature\n- [ ] ship it\n\n## Next Steps\n- launch\n\nBlocker: need review\n\nDecision: use SQLite\n").unwrap();

        let result = parse_agent_files(&dir);
        let _ = std::fs::remove_dir_all(&dir);

        assert!(result.is_some());
        let ctx = result.unwrap();
        assert_eq!(ctx.description, Some("A full featured project.".to_string()));
        assert_eq!(ctx.current_phase, Some("Beta".to_string()));
        assert_eq!(ctx.checklist_total, 3);
        assert_eq!(ctx.checklist_done, 1);
        assert!(ctx.next_steps.contains(&"build feature".to_string()));
        assert!(ctx.blockers.iter().any(|b| b.contains("review")));
        assert!(!ctx.recent_decisions.is_empty());
    }
}

/// Parse a single agent file for structured content
fn parse_agent_content(content: &str, context: &mut AgentContext) {
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Extract phase from headings like "## Phase" or "### Current Phase"
        if line.starts_with('#') {
            let lower = line.to_lowercase();
            if lower.contains("phase") {
                // Skip blank lines to find the phase description
                for next in lines.iter().skip(i + 1) {
                    let next = next.trim();
                    if next.is_empty() {
                        continue;
                    }
                    if !next.starts_with('#') {
                        context.current_phase = Some(next.to_string());
                    }
                    break;
                }
            }
            if lower.contains("task") || lower.contains("objective") || lower.contains("goal") {
                for next in lines.iter().skip(i + 1) {
                    let next = next.trim();
                    if next.is_empty() {
                        continue;
                    }
                    if !next.starts_with('#') {
                        context.current_task = Some(next.to_string());
                    }
                    break;
                }
            }
        }

        // Extract next steps / todos
        if line.starts_with("- [") || line.starts_with("* [") {
            let checked = line.contains("[x]") || line.contains("[X]");
            let item_text = extract_checklist_item(line);
            if let Some(text) = item_text {
                if checked {
                    context.completed_items.push(text);
                    context.checklist_done += 1;
                } else {
                    context.next_steps.push(text);
                }
                context.checklist_total += 1;
            }
        }

        // Also handle numbered lists that look like steps
        if line.starts_with(|c: char| c.is_ascii_digit())
            && line.contains('.')
            && !line.contains('#')
            && line.len() > 3
        {
            let after_dot = line.split_once('.').map(|x| x.1).unwrap_or("").trim();
            if !after_dot.is_empty()
                && after_dot.len() > 5
                && !after_dot.starts_with(' ')
            {
                // Might be a "next step" style line
                let lower = line.to_lowercase();
                if lower.contains("next") || lower.contains("todo") || lower.contains("step") {
                    // Skip it's likely a header
                } else {
                    context.next_steps.push(after_dot.to_string());
                }
            }
        }

        // Extract blockers
        if line.to_lowercase().contains("blocker")
            || line.to_lowercase().contains("blocked")
            || line.to_lowercase().contains("blocking")
        {
            // Check if there's content after the colon
            if let Some(after_colon) = line.split_once(':').map(|x| x.1) {
                let text = after_colon.trim();
                if !text.is_empty() {
                    context.blockers.push(text.to_string());
                }
            } else {
                // No colon content, check next non-blank line
                for next in lines.iter().skip(i + 1) {
                    let next = next.trim();
                    if next.is_empty() {
                        continue;
                    }
                    if !next.starts_with('#') {
                        context.blockers.push(next.to_string());
                    }
                    break;
                }
            }
        }

        // Extract "Next Steps" section - collect items after this heading
        let lower_line = line.to_lowercase();
        if lower_line.contains("next steps")
            || lower_line.contains("up next")
            || lower_line.contains("what's next")
        {
            let mut j = i + 1;
            while j < lines.len() {
                let next_line = lines[j].trim();
                if next_line.is_empty() {
                    j += 1;
                    continue;
                }
                if next_line.starts_with('#') {
                    break;
                }
                // Collect bullet points
                if next_line.starts_with("- ") || next_line.starts_with("* ") {
                    let item = next_line.trim_start_matches("- ")
                        .trim_start_matches("* ")
                        .trim();
                    if !item.is_empty() {
                        context.next_steps.push(item.to_string());
                    }
                }
                j += 1;
            }
        }

        // Extract decisions
        if lower_line.contains("decision")
            || lower_line.contains("chose")
            || lower_line.contains("decided")
        {
            if let Some(after_colon) = line.split_once(':').map(|x| x.1) {
                let text = after_colon.trim();
                if !text.is_empty() {
                    context.recent_decisions.push(text.to_string());
                }
            }
        }

        i += 1;
    }

    // Deduplicate next steps
    context.next_steps.sort();
    context.next_steps.dedup();

    // Limit to 20 items to keep things manageable
    context.next_steps.truncate(20);
    context.blockers.truncate(10);
}

/// Extract the text of a checklist item like "- [ ] do something" -> "do something"
fn extract_checklist_item(line: &str) -> Option<String> {
    let line = line.trim();
    // Remove the bullet and checkbox marker
    if let Some(rest) = line.strip_prefix("- [") {
        let after = rest.split_once(']')?.1;
        let text = after.trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    if let Some(rest) = line.strip_prefix("* [") {
        let after = rest.split_once(']')?.1;
        let text = after.trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    None
}
