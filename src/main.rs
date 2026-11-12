use clap::{Parser as ClapParser, Subcommand};
use rtsc_core::codegen_native::NativeCodegen;
use rtsc_core::codegen_ts_sdk::TsSdkCodegen;
use rtsc_core::codegen_unity::UnityCodegen;
use rtsc_core::codegen_unreal::UnrealCodegen;
use rtsc_core::compile_and_check;
use rtsc_core::gbnf_exporter::export_rustts_gbnf;
use std::fs;
use std::path::PathBuf;
use std::process;

#[derive(ClapParser)]
#[command(
    name = "rtsc",
    version = "1.0.0-rts",
    about = "RustTS (.rts) Deterministic Systems & Engine Compiler for Humans and AI"
)]
struct Cli {
    /// Export the formal GBNF grammar for LLM Constrained Decoding (llama.cpp, vLLM)
    #[arg(long = "export-gbnf")]
    export_gbnf: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate `.rts` source code capabilities, types, and arena scopes (< 15ms)
    Check {
        /// Path to the `.rts` file
        file: PathBuf,
        /// Emit structured JSON AST diagnostics for AI self-correction
        #[arg(long = "json-diagnostics")]
        json_diagnostics: bool,
    },
    /// Transpile and compile `.rts` source code to the target platform
    Build {
        /// Path to the `.rts` file
        file: PathBuf,
        /// Compilation target: `native` (C99/SIMD/SoA), `unreal` (UE5 UHT .h/.cpp), `unity` (C-ABI + C# NativeArray), or `ts-sdk`
        #[arg(long = "target", default_value = "native")]
        target: String,
        /// Emit TypeScript SDK contract (`client.ts`) for Next.js/React frontends
        #[arg(long = "emit-ts-sdk")]
        emit_ts_sdk: bool,
        /// Emit structured JSON AST diagnostics on failure
        #[arg(long = "json-diagnostics")]
        json_diagnostics: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    if cli.export_gbnf {
        println!("{}", export_rustts_gbnf());
        return;
    }

    match cli.command {
        Some(Commands::Check {
            file,
            json_diagnostics,
        }) => {
            let file_str = file.to_string_lossy().replace('\\', "/");
            let source = fs::read_to_string(&file).unwrap_or_else(|err| {
                eprintln!("Failed to read '{}': {}", file_str, err);
                process::exit(1);
            });

            let (_module, report) = compile_and_check(&source, &file_str);
            if json_diagnostics {
                println!("{}", report.to_json_pretty());
            } else {
                println!("{}", report.to_human_terminal());
            }

            if report.status != "ok" {
                process::exit(1);
            }
        }
        Some(Commands::Build {
            file,
            target,
            emit_ts_sdk,
            json_diagnostics,
        }) => {
            let file_str = file.to_string_lossy().replace('\\', "/");
            let source = fs::read_to_string(&file).unwrap_or_else(|err| {
                eprintln!("Failed to read '{}': {}", file_str, err);
                process::exit(1);
            });

            let (module_opt, report) = compile_and_check(&source, &file_str);
            if report.status != "ok" {
                if json_diagnostics {
                    println!("{}", report.to_json_pretty());
                } else {
                    eprintln!("{}", report.to_human_terminal());
                }
                process::exit(1);
            }

            let module = module_opt.expect("Valid module AST");
            if emit_ts_sdk || target == "ts-sdk" {
                println!("{}", TsSdkCodegen::emit_typescript_sdk(&module));
                return;
            }
            match target.as_str() {
                "native" | "c99" => {
                    println!("{}", NativeCodegen::emit_c99(&module));
                }
                "unreal" | "unreal-cpp" => {
                    for artifact in UnrealCodegen::emit_unreal_actor(&module) {
                        println!("// === FILE: {} ===\n{}", artifact.header_filename, artifact.header_code);
                        println!("// === FILE: {} ===\n{}", artifact.source_filename, artifact.source_code);
                    }
                }
                "unity" | "unity-burst" => {
                    let out = UnityCodegen::emit_unity_bindings(&module);
                    println!("// === RUST C-ABI PLUGIN ===\n{}", out.rust_ffi_code);
                    println!("// === C# NATIVEARRAY WRAPPER ===\n{}", out.csharp_wrapper_code);
                }
                other => {
                    eprintln!("Unsupported target '{}'. Valid targets: native, unreal, unity", other);
                    process::exit(1);
                }
            }
        }
        None => {
            eprintln!("Usage: rtsc check <file.rts> [--json-diagnostics] | rtsc build <file.rts> --target=<native|unreal|unity> | rtsc --export-gbnf");
            process::exit(1);
        }
    }
}

