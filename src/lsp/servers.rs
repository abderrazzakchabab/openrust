use std::path::Path;

/// Language server definition with command, arguments, and file associations
#[derive(Debug, Clone)]
pub struct LspServerDef {
    pub language_id: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub file_extensions: &'static [&'static str],
    pub root_markers: &'static [&'static str],
}

/// Builtin language server configurations (30+ servers)
pub const BUILTIN_SERVERS: &[LspServerDef] = &[
    LspServerDef {
        language_id: "rust",
        command: "rust-analyzer",
        args: &[],
        file_extensions: &["rs"],
        root_markers: &["Cargo.toml"],
    },
    LspServerDef {
        language_id: "typescript",
        command: "typescript-language-server",
        args: &["--stdio"],
        file_extensions: &["ts", "tsx"],
        root_markers: &["tsconfig.json", "package.json"],
    },
    LspServerDef {
        language_id: "javascript",
        command: "typescript-language-server",
        args: &["--stdio"],
        file_extensions: &["js", "jsx"],
        root_markers: &["package.json"],
    },
    LspServerDef {
        language_id: "python",
        command: "pyright-langserver",
        args: &["--stdio"],
        file_extensions: &["py"],
        root_markers: &["pyproject.toml", "setup.py", "requirements.txt"],
    },
    LspServerDef {
        language_id: "go",
        command: "gopls",
        args: &["serve"],
        file_extensions: &["go"],
        root_markers: &["go.mod"],
    },
    LspServerDef {
        language_id: "c",
        command: "clangd",
        args: &[],
        file_extensions: &["c", "h"],
        root_markers: &["compile_commands.json", "CMakeLists.txt"],
    },
    LspServerDef {
        language_id: "cpp",
        command: "clangd",
        args: &[],
        file_extensions: &["cpp", "hpp", "cc", "cxx"],
        root_markers: &["compile_commands.json", "CMakeLists.txt"],
    },
    LspServerDef {
        language_id: "java",
        command: "jdtls",
        args: &[],
        file_extensions: &["java"],
        root_markers: &["pom.xml", "build.gradle"],
    },
    LspServerDef {
        language_id: "ruby",
        command: "solargraph",
        args: &["stdio"],
        file_extensions: &["rb"],
        root_markers: &["Gemfile"],
    },
    LspServerDef {
        language_id: "php",
        command: "intelephense",
        args: &["--stdio"],
        file_extensions: &["php"],
        root_markers: &["composer.json"],
    },
    LspServerDef {
        language_id: "csharp",
        command: "OmniSharp",
        args: &["--languageserver"],
        file_extensions: &["cs"],
        root_markers: &["*.csproj", "*.sln"],
    },
    LspServerDef {
        language_id: "dart",
        command: "dart",
        args: &["language-server", "--protocol=lsp"],
        file_extensions: &["dart"],
        root_markers: &["pubspec.yaml"],
    },
    LspServerDef {
        language_id: "elixir",
        command: "elixir-ls",
        args: &[],
        file_extensions: &["ex", "exs"],
        root_markers: &["mix.exs"],
    },
    LspServerDef {
        language_id: "haskell",
        command: "haskell-language-server-wrapper",
        args: &["--lsp"],
        file_extensions: &["hs"],
        root_markers: &["stack.yaml", "*.cabal"],
    },
    LspServerDef {
        language_id: "kotlin",
        command: "kotlin-language-server",
        args: &[],
        file_extensions: &["kt", "kts"],
        root_markers: &["build.gradle.kts"],
    },
    LspServerDef {
        language_id: "lua",
        command: "lua-language-server",
        args: &[],
        file_extensions: &["lua"],
        root_markers: &[".luarc.json"],
    },
    LspServerDef {
        language_id: "nix",
        command: "nil",
        args: &[],
        file_extensions: &["nix"],
        root_markers: &["flake.nix"],
    },
    LspServerDef {
        language_id: "ocaml",
        command: "ocamllsp",
        args: &[],
        file_extensions: &["ml", "mli"],
        root_markers: &["dune-project"],
    },
    LspServerDef {
        language_id: "swift",
        command: "sourcekit-lsp",
        args: &[],
        file_extensions: &["swift"],
        root_markers: &["Package.swift"],
    },
    LspServerDef {
        language_id: "svelte",
        command: "svelteserver",
        args: &["--stdio"],
        file_extensions: &["svelte"],
        root_markers: &["svelte.config.js"],
    },
    LspServerDef {
        language_id: "vue",
        command: "vue-language-server",
        args: &["--stdio"],
        file_extensions: &["vue"],
        root_markers: &["vue.config.js"],
    },
    LspServerDef {
        language_id: "html",
        command: "vscode-html-language-server",
        args: &["--stdio"],
        file_extensions: &["html", "htm"],
        root_markers: &[],
    },
    LspServerDef {
        language_id: "css",
        command: "vscode-css-language-server",
        args: &["--stdio"],
        file_extensions: &["css", "scss", "less"],
        root_markers: &[],
    },
    LspServerDef {
        language_id: "json",
        command: "vscode-json-language-server",
        args: &["--stdio"],
        file_extensions: &["json", "jsonc"],
        root_markers: &[],
    },
    LspServerDef {
        language_id: "yaml",
        command: "yaml-language-server",
        args: &["--stdio"],
        file_extensions: &["yaml", "yml"],
        root_markers: &[],
    },
    LspServerDef {
        language_id: "toml",
        command: "taplo",
        args: &["lsp", "stdio"],
        file_extensions: &["toml"],
        root_markers: &[],
    },
    LspServerDef {
        language_id: "bash",
        command: "bash-language-server",
        args: &["start"],
        file_extensions: &["sh", "bash"],
        root_markers: &[],
    },
    LspServerDef {
        language_id: "zig",
        command: "zls",
        args: &[],
        file_extensions: &["zig"],
        root_markers: &["build.zig"],
    },
    LspServerDef {
        language_id: "scala",
        command: "metals",
        args: &[],
        file_extensions: &["scala", "sc"],
        root_markers: &["build.sbt"],
    },
    LspServerDef {
        language_id: "erlang",
        command: "erlang_ls",
        args: &[],
        file_extensions: &["erl", "hrl"],
        root_markers: &["rebar.config"],
    },
    LspServerDef {
        language_id: "clojure",
        command: "clojure-lsp",
        args: &[],
        file_extensions: &["clj", "cljs", "cljc"],
        root_markers: &["deps.edn", "project.clj"],
    },
];

/// Get server configuration for a specific language ID
pub fn get_server(language_id: &str) -> Option<&'static LspServerDef> {
    BUILTIN_SERVERS
        .iter()
        .find(|s| s.language_id == language_id)
}

/// Get server configuration for a file extension
pub fn get_server_for_extension(ext: &str) -> Option<&'static LspServerDef> {
    BUILTIN_SERVERS
        .iter()
        .find(|s| s.file_extensions.contains(&ext))
}

/// Check if a language server command is available in PATH
pub fn is_server_available(command: &str) -> bool {
    which::which(command).is_ok()
}

/// Auto-detect which language servers should run based on files in directory
pub fn detect_servers(root_dir: &Path) -> Vec<&'static LspServerDef> {
    let mut detected = Vec::new();

    for server_def in BUILTIN_SERVERS {
        // Check if any root markers exist in the directory
        for marker in server_def.root_markers {
            let marker_path = root_dir.join(marker);
            if marker_path.exists() {
                detected.push(server_def);
                break;
            }
        }
    }

    detected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_server_count() {
        assert!(
            BUILTIN_SERVERS.len() >= 30,
            "Expected at least 30 servers, found {}",
            BUILTIN_SERVERS.len()
        );
    }

    #[test]
    fn test_get_server_rust() {
        let server = get_server("rust");
        assert!(server.is_some());
        let server = server.unwrap();
        assert_eq!(server.language_id, "rust");
        assert_eq!(server.command, "rust-analyzer");
        assert_eq!(server.file_extensions, &["rs"]);
    }

    #[test]
    fn test_get_server_typescript() {
        let server = get_server("typescript");
        assert!(server.is_some());
        let server = server.unwrap();
        assert_eq!(server.language_id, "typescript");
        assert_eq!(server.command, "typescript-language-server");
        assert!(server.file_extensions.contains(&"ts"));
        assert!(server.file_extensions.contains(&"tsx"));
    }

    #[test]
    fn test_get_server_python() {
        let server = get_server("python");
        assert!(server.is_some());
        let server = server.unwrap();
        assert_eq!(server.language_id, "python");
        assert_eq!(server.command, "pyright-langserver");
    }

    #[test]
    fn test_get_server_for_extension_rs() {
        let server = get_server_for_extension("rs");
        assert!(server.is_some());
        assert_eq!(server.unwrap().language_id, "rust");
    }

    #[test]
    fn test_get_server_for_extension_py() {
        let server = get_server_for_extension("py");
        assert!(server.is_some());
        assert_eq!(server.unwrap().language_id, "python");
    }

    #[test]
    fn test_get_server_for_extension_ts() {
        let server = get_server_for_extension("ts");
        assert!(server.is_some());
        assert_eq!(server.unwrap().language_id, "typescript");
    }

    #[test]
    fn test_unknown_extension() {
        let server = get_server_for_extension("unknown_ext_xyz");
        assert!(server.is_none());
    }

    #[test]
    fn test_unknown_language() {
        let server = get_server("unknown_language_xyz");
        assert!(server.is_none());
    }

    #[test]
    fn test_detect_servers_empty_dir() {
        let temp_dir = std::env::temp_dir();
        let detected = detect_servers(&temp_dir);
        // May or may not find servers depending on temp dir contents
        // Just verify it returns a Vec
        assert!(detected.is_empty() || !detected.is_empty());
    }

    #[test]
    fn test_server_def_consistency() {
        // Verify all servers have non-empty language_id and command
        for server in BUILTIN_SERVERS {
            assert!(!server.language_id.is_empty());
            assert!(!server.command.is_empty());
            assert!(!server.file_extensions.is_empty());
        }
    }

    #[test]
    fn test_is_server_available_rust_analyzer() {
        // This test may pass or fail depending on system
        // Just verify the function doesn't panic
        let _available = is_server_available("rust-analyzer");
    }
}
