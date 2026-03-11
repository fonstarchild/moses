/// Unit tests for the Moses agent core logic.
/// Run with: cargo test

#[cfg(test)]
mod code_block_extraction {
    // Inline the extraction logic so we can test it without Tauri running.

    fn extract_all_code_blocks(text: &str) -> Vec<(String, String)> {
        let mut results = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find("```") {
            let after = &rest[start + 3..];
            let (lang, content_start) = match after.find('\n') {
                Some(nl) => (after[..nl].trim().to_string(), nl + 1),
                None => break,
            };
            let content = &after[content_start..];
            match content.find("\n```") {
                Some(end) => {
                    let body = content[..end].to_string();
                    let real_lines = body.lines().filter(|l| !l.trim().is_empty()).count();
                    if real_lines >= 3 {
                        results.push((lang, body));
                    }
                    rest = &content[end + 4..];
                }
                None => break,
            }
        }
        results
    }

    #[test]
    fn extracts_single_block() {
        let text =
            "Here is the code:\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\nDone.";
        let blocks = extract_all_code_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "rust");
        assert!(blocks[0].1.contains("println!"));
    }

    #[test]
    fn extracts_multiple_blocks() {
        let text = "First:\n```python\ndef foo():\n    pass\n    return 1\n```\nSecond:\n```typescript\nfunction bar(): void {\n    console.log('hi');\n    return;\n}\n```";
        let blocks = extract_all_code_blocks(text);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, "python");
        assert_eq!(blocks[1].0, "typescript");
    }

    #[test]
    fn skips_incomplete_block() {
        // No closing ``` — model was cut off, should not be saved
        let text = "```rust\nfn incomplete() {\n    // never finished";
        let blocks = extract_all_code_blocks(text);
        assert!(blocks.is_empty());
    }

    #[test]
    fn skips_tiny_snippet() {
        // Only 2 lines — too small to be a file
        let text = "Use this:\n```rust\nlet x = 1;\nlet y = 2;\n```";
        let blocks = extract_all_code_blocks(text);
        assert!(blocks.is_empty());
    }

    #[test]
    fn handles_no_language_tag() {
        let text = "```\nline one\nline two\nline three\nline four\n```";
        let blocks = extract_all_code_blocks(text);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "");
    }
}

#[cfg(test)]
mod special_token_stripping {
    fn strip_special_tokens(s: String) -> String {
        const JUNK: &[&str] = &[
            "<｜begin▁of▁sentence｜>",
            "<｜end▁of▁sentence｜>",
            "<｜fim▁begin｜>",
            "<｜fim▁end｜>",
            "<｜fim▁hole｜>",
            "<|begin_of_sentence|>",
            "<|end_of_sentence|>",
        ];
        let mut out = s;
        for pat in JUNK {
            if out.contains(pat) {
                out = out.replace(pat, "");
            }
        }
        out
    }

    #[test]
    fn strips_begin_of_sentence() {
        let input = "hello<｜begin▁of▁sentence｜>world".to_string();
        assert_eq!(strip_special_tokens(input), "helloworld");
    }

    #[test]
    fn strips_end_of_sentence() {
        let input = "some code<｜end▁of▁sentence｜>".to_string();
        assert_eq!(strip_special_tokens(input), "some code");
    }

    #[test]
    fn strips_multiple_tokens() {
        let input = "<｜begin▁of▁sentence｜>code<｜end▁of▁sentence｜>".to_string();
        assert_eq!(strip_special_tokens(input), "code");
    }

    #[test]
    fn clean_input_unchanged() {
        let input = "fn main() { println!(\"hello\"); }".to_string();
        let expected = input.clone();
        assert_eq!(strip_special_tokens(input), expected);
    }

    #[test]
    fn strips_ascii_variant() {
        let input = "start<|begin_of_sentence|>end".to_string();
        assert_eq!(strip_special_tokens(input), "startend");
    }
}

#[cfg(test)]
mod file_path_inference {
    fn infer_test_file_path(source_path: &str) -> String {
        use std::path::Path;
        let p = Path::new(source_path);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("txt");
        let dir = p
            .parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_default();
        match ext {
            "rs" => format!("{}/{}_test.rs", dir, stem),
            "ts" | "tsx" => format!("{}/{}.test.ts", dir, stem),
            "js" | "jsx" => format!("{}/{}.test.js", dir, stem),
            "py" => format!("{}/test_{}.py", dir, stem),
            "go" => format!("{}/{}_test.go", dir, stem),
            _ => format!("{}/{}_test.{}", dir, stem, ext),
        }
    }

    #[test]
    fn rust_test_file() {
        let path = infer_test_file_path("/project/src/analytics.rs");
        assert_eq!(path, "/project/src/analytics_test.rs");
    }

    #[test]
    fn typescript_test_file() {
        let path = infer_test_file_path("/project/src/utils.ts");
        assert_eq!(path, "/project/src/utils.test.ts");
    }

    #[test]
    fn python_test_file() {
        let path = infer_test_file_path("/project/backend/analytics.py");
        assert_eq!(path, "/project/backend/test_analytics.py");
    }

    #[test]
    fn go_test_file() {
        let path = infer_test_file_path("/project/cmd/server.go");
        assert_eq!(path, "/project/cmd/server_test.go");
    }

    #[test]
    fn javascript_test_file() {
        let path = infer_test_file_path("/app/src/components/Button.jsx");
        assert_eq!(path, "/app/src/components/Button.test.js");
    }
}

#[cfg(test)]
mod chunking {
    use crate::workspace::vector_store::CodeChunk;

    fn chunk_by_lines(content: &str, file: &str, size: usize) -> Vec<CodeChunk> {
        content
            .lines()
            .collect::<Vec<_>>()
            .chunks(size)
            .enumerate()
            .filter_map(|(i, lines)| {
                let text = lines.join("\n");
                if text.trim().len() > 10 {
                    Some(CodeChunk {
                        file: file.to_string(),
                        line: i * size + 1,
                        text,
                        node_kind: "lines".to_string(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    #[test]
    fn chunks_small_file_into_one() {
        let content = "fn a() {}\nfn b() {}\nfn c() {}";
        let chunks = chunk_by_lines(content, "test.rs", 60);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].line, 1);
    }

    #[test]
    fn chunks_large_file_into_multiple() {
        let content = (0..200)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunk_by_lines(&content, "big.rs", 60);
        assert!(chunks.len() >= 3);
        assert_eq!(chunks[0].line, 1);
        assert_eq!(chunks[1].line, 61);
    }

    #[test]
    fn skips_empty_chunks() {
        let content = "\n\n\n\n";
        let chunks = chunk_by_lines(content, "empty.rs", 60);
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_has_correct_file_ref() {
        let content = "fn foo() {}\nfn bar() {}\nfn baz() {}";
        let chunks = chunk_by_lines(content, "src/main.rs", 60);
        assert_eq!(chunks[0].file, "src/main.rs");
    }
}
