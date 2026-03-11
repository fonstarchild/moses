/// Returns (system_prompt, instruction) where instruction is appended to the user message.
/// deepseek-coder:6.7b ignores the system field, so we put real instructions in the user turn.
pub fn build_system_prompt(mode: &str, _context: &str) -> String {
    // Minimal system prompt — real instructions go into the user message via build_instruction()
    "You are Moses, a local AI coding assistant. Answer concisely and accurately.".to_string()
}

pub fn build_instruction(mode: &str) -> &'static str {
    match mode {
        "Edit" => "Apply the requested change to the file shown above. Output ONLY a unified diff in this format, nothing else:\n```diff\n--- a/filename\n+++ b/filename\n@@ -LINE,COUNT +LINE,COUNT @@\n context\n-removed\n+added\n```",
        "Explain" => "Explain the code shown above. Cover: what it does, how it works, key functions, and any important details. Be specific and reference actual names from the code.",
        "Refactor" => "Refactor the code shown above. First briefly explain what you're improving, then output a unified diff.",
        "BugFix" => "Find and fix the bug in the code above. Explain the root cause in one sentence, then output a unified diff with the fix.",
        "TestGen" => "Write tests for the code shown above. Use the same language and test framework as the project. Output complete, runnable test code.",
        _ => "Answer the question about the code shown above. Be specific and reference actual code when relevant.",
    }
}
