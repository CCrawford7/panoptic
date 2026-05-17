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
                // Get the next non-empty line as the phase description
                if let Some(next) = lines.get(i + 1) {
                    let next = next.trim();
                    if !next.is_empty() && !next.starts_with('#') {
                        context.current_phase = Some(next.to_string());
                    }
                }
            }
            if lower.contains("task") || lower.contains("objective") || lower.contains("goal") {
                if let Some(next) = lines.get(i + 1) {
                    let next = next.trim();
                    if !next.is_empty() && !next.starts_with('#') {
                        context.current_task = Some(next.to_string());
                    }
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
            let after_dot = line.splitn(2, '.').nth(1).unwrap_or("").trim();
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
            if let Some(after_colon) = line.splitn(2, ':').nth(1) {
                let text = after_colon.trim();
                if !text.is_empty() {
                    context.blockers.push(text.to_string());
                }
            } else if let Some(next) = lines.get(i + 1) {
                let next = next.trim();
                if !next.is_empty() && !next.starts_with('#') {
                    context.blockers.push(next.to_string());
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
            if let Some(after_colon) = line.splitn(2, ':').nth(1) {
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
        let after = rest.splitn(2, ']').nth(1)?;
        let text = after.trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    if let Some(rest) = line.strip_prefix("* [") {
        let after = rest.splitn(2, ']').nth(1)?;
        let text = after.trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    None
}
