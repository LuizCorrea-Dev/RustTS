use clap::{Parser as ClapParser, Subcommand};
use rtsc_core::codegen_gpu::GpuCodegen;
use rtsc_core::codegen_llvm::LlvmIrCodegen;
use rtsc_core::codegen_mlir::MlirCodegen;
use rtsc_core::codegen_native::NativeCodegen;
use rtsc_core::codegen_wasi::{WasiCodegen, DEFAULT_FUEL_LIMIT};
use rtsc_core::codegen_ts_sdk::TsSdkCodegen;
use rtsc_core::codegen_unity::UnityCodegen;
use rtsc_core::codegen_unreal::UnrealCodegen;
use rtsc_core::compile_and_check_project;
use rtsc_core::gbnf_exporter::export_rustts_gbnf;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

pub const RTSC_VERSION: &str = "2.1.0-rts";

#[derive(ClapParser)]
#[command(
    name = "rtsc",
    version = RTSC_VERSION,
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
    /// Initialize a new RustTS project structure (`rts.toml`, `src/main.rts`, `build/`)
    Init {
        /// Target directory or project name (defaults to current directory `.`)
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Validate `.rts` source code capabilities, types, and arena scopes (< 15ms)
    Check {
        /// Path to the `.rts` file
        file: PathBuf,
        /// Emit structured JSON AST diagnostics for AI self-correction
        #[arg(long = "json-diagnostics")]
        json_diagnostics: bool,
    },
    /// Transpile and compile `.rts` source code to the target platform or native executable
    Build {
        /// Path to the `.rts` file
        file: PathBuf,
        /// Compilation target: `native` (C99/SIMD/SoA), `unreal` (UE5 UHT .h/.cpp), `unity` (C-ABI + C# NativeArray), or `ts-sdk`
        #[arg(long = "target", default_value = "native")]
        target: String,
        /// Write output directly to UTF-8 file (without BOM) instead of stdout
        #[arg(short = 'o', long = "output")]
        output: Option<PathBuf>,
        /// Output directory for multi-file targets (`unreal` .h/.cpp, `unity` .rs/.cs, `native` .c/.exe)
        #[arg(long = "out-dir")]
        out_dir: Option<PathBuf>,
        /// Compile generated C99 directly into a native executable binary via Clang/LLVM
        #[arg(long = "bin")]
        compile_bin: bool,
        /// Emit TypeScript SDK contract (`client.ts`) for Next.js/React frontends
        #[arg(long = "emit-ts-sdk")]
        emit_ts_sdk: bool,
        /// Emit C/C++ ABI header (`<module>.h`) for public `.rts` functions
        #[arg(long = "emit-header")]
        emit_header: bool,
        /// Emit debug symbols (`-g -O0` for LLDB/GDB source mapping via `#line`)
        #[arg(short = 'g', long = "debug")]
        debug_symbols: bool,
        /// Emit structured JSON AST diagnostics on failure
        #[arg(long = "json-diagnostics")]
        json_diagnostics: bool,
        /// Link against external native C library (e.g., `-l raylib -l opengl32`)
        #[arg(short = 'l', long = "link-lib")]
        link_libs: Vec<String>,
        /// Add directory to native library search path (`-L <dir>`)
        #[arg(short = 'L', long = "lib-dir")]
        lib_dirs: Vec<PathBuf>,
        /// Add directory to C header include search path (`-I <dir>`)
        #[arg(short = 'I', long = "include-dir")]
        include_dirs: Vec<PathBuf>,
        /// Deterministic WASI sandbox fuel budget (`--fuel-limit <N>`, default: 100000)
        #[arg(long = "fuel-limit", default_value_t = DEFAULT_FUEL_LIMIT)]
        fuel_limit: u64,
    },
    /// Compile `.rts` to a native SIMD binary and execute it immediately (C99 or Direct LLVM IR via `--llvm`)
    Run {
        /// Path to the `.rts` file containing `pub fn main()`
        file: PathBuf,
        /// Lower directly to LLVM IR (`.ll`) and compile with `clang -x ir` (bypasses C99)
        #[arg(long = "llvm")]
        llvm: bool,
        /// Emit debug symbols (`-g -O0` for LLDB/GDB source mapping via `#line`)
        #[arg(short = 'g', long = "debug")]
        debug_symbols: bool,
        /// Emit structured JSON AST diagnostics on failure
        #[arg(long = "json-diagnostics")]
        json_diagnostics: bool,
        /// Link against external native C library (e.g., `-l raylib -l opengl32`)
        #[arg(short = 'l', long = "link-lib")]
        link_libs: Vec<String>,
        /// Add directory to native library search path (`-L <dir>`)
        #[arg(short = 'L', long = "lib-dir")]
        lib_dirs: Vec<PathBuf>,
        /// Add directory to C header include search path (`-I <dir>`)
        #[arg(short = 'I', long = "include-dir")]
        include_dirs: Vec<PathBuf>,
        /// Deterministic WASI sandbox fuel budget (`--fuel-limit <N>`, default: 100000)
        #[arg(long = "fuel-limit", default_value_t = DEFAULT_FUEL_LIMIT)]
        fuel_limit: u64,
        /// Extra arguments passed through to the compiled binary after `--` (e.g. `-- --smoke-test`)
        #[arg(last = true)]
        run_args: Vec<String>,
    },
}

fn find_clang_binary() -> Option<PathBuf> {
    if Command::new("clang").arg("--version").output().is_ok() {
        return Some(PathBuf::from("clang"));
    }
    let candidates = [
        r"C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\Llvm\x64\bin\clang.exe",
        r"C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\Llvm\x64\bin\clang.exe",
        r"C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Tools\Llvm\x64\bin\clang.exe",
        r"C:\Program Files\Microsoft Visual Studio\2022\BuildTools\VC\Tools\Llvm\x64\bin\clang.exe",
        r"C:\Program Files\LLVM\bin\clang.exe",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn write_utf8_no_bom(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    fs::write(path, content.as_bytes()).unwrap_or_else(|err| {
        eprintln!("Failed to write '{}': {}", path.display(), err);
        process::exit(1);
    });
}

fn append_platform_hal_libs(cmd: &mut Command) {
    #[cfg(target_os = "windows")]
    {
        cmd.arg("-luser32").arg("-lgdi32").arg("-lwinmm");
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = cmd;
    }
}

fn compile_c99_with_clang(
    c_file: &Path,
    bin_file: &Path,
    include_dirs: &[PathBuf],
    lib_dirs: &[PathBuf],
    link_libs: &[String],
    debug_symbols: bool,
) {
    let clang = find_clang_binary().unwrap_or_else(|| {
        eprintln!("error: LLVM/Clang compiler ('clang') not found in PATH or Visual Studio LLVM tools.");
        process::exit(1);
    });

    if let Some(parent) = bin_file.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }

    let mut cmd = Command::new(&clang);
    let flags: &[&str] = if debug_symbols {
        &["-std=c11", "-g", "-O0", "-mavx2", "-mfma", "-Wall", "-Wextra", "-Werror"]
    } else {
        &["-std=c11", "-O3", "-mavx2", "-mfma", "-Wall", "-Wextra", "-Werror"]
    };
    cmd.args(flags)
        .arg(c_file)
        .arg("-o")
        .arg(bin_file);

    for inc in include_dirs {
        cmd.arg(format!("-I{}", inc.display()));
    }
    for lib_dir in lib_dirs {
        cmd.arg(format!("-L{}", lib_dir.display()));
    }
    for lib in link_libs {
        cmd.arg(format!("-l{}", lib));
    }
    append_platform_hal_libs(&mut cmd);

    let status = cmd.status().unwrap_or_else(|err| {
        eprintln!("Failed to invoke '{}': {}", clang.display(), err);
        process::exit(1);
    });

    if !status.success() {
        eprintln!("error: native C99 compilation failed via '{}'.", clang.display());
        process::exit(status.code().unwrap_or(1));
    }
}

fn compile_llvm_ir_with_clang(
    ll_file: &Path,
    bin_file: &Path,
    lib_dirs: &[PathBuf],
    link_libs: &[String],
    debug_symbols: bool,
) {
    let Some(clang) = find_clang_binary() else {
        eprintln!("error: LLVM/Clang ('clang') not found in PATH or standard installation paths.");
        eprintln!("hint: install LLVM (winget install LLVM.LLVM) to compile LLVM IR (.ll) directly to a native binary.");
        process::exit(1);
    };

    let mut cmd = Command::new(&clang);
    let flags: &[&str] = if debug_symbols {
        &["-x", "ir", "-g", "-O0", "-mavx2", "-mfma", "-Wno-override-module"]
    } else {
        &["-x", "ir", "-O3", "-mavx2", "-mfma", "-Wno-override-module"]
    };
    cmd.args(flags)
        .arg(ll_file)
        .arg("-o")
        .arg(bin_file);

    for lib_dir in lib_dirs {
        cmd.arg(format!("-L{}", lib_dir.display()));
    }
    for lib in link_libs {
        cmd.arg(format!("-l{}", lib));
    }
    append_platform_hal_libs(&mut cmd);

    let status = cmd.status().unwrap_or_else(|err| {
        eprintln!("Failed to invoke '{}': {}", clang.display(), err);
        process::exit(1);
    });

    if !status.success() {
        eprintln!("error: direct LLVM IR (.ll) compilation failed via '{}'.", clang.display());
        process::exit(status.code().unwrap_or(1));
    }
}

fn scaffold_project(target_dir: &Path) {
    let src_dir = target_dir.join("src");
    let build_dir = target_dir.join("build");
    let _ = fs::create_dir_all(&src_dir);
    let _ = fs::create_dir_all(&build_dir);

    let project_name = target_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("meu_projeto_rts");

    let rts_toml = format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\ncompiler = \"2.1.0-rts\"\ntarget = \"native\"\n",
        project_name
    );
    write_utf8_no_bom(&target_dir.join("rts.toml"), &rts_toml);

    let gitignore = "/build/\n*.exe\n*.pdb\n*.obj\n*.o\n";
    write_utf8_no_bom(&target_dir.join(".gitignore"), gitignore);

    let main_rts = r#"module app.main;
import core.hardware;
import core.math;

struct Limites {
    max_temp: f32,
}

enum FalhaSensor {
    Offline(u8),
    Critico(f32),
}

@sealed_sig
pub fn monitorar(
    lim: Limites,
    i2c: cap I2CBus<0x48>,
    cooler: cap PWMPin<4>
) -> Result<f32, FalhaSensor> {
    let bruto: u16 = i2c.read_register_u16(0x00)
        .map_err(|_| FalhaSensor::Offline(0x48))?;

    let temp: f32 = (bruto as f32) * 0.0625;
    if temp > lim.max_temp {
        cooler.set_duty(100);
        return Err(FalhaSensor::Critico(temp));
    }

    let duty: u8 = math::clamp(temp, 20.0, 80.0) as u8;
    cooler.set_duty(duty);
    return Ok(temp);
}

pub fn main() -> i32 {
    let lim: Limites = Limites { max_temp: 85.0 };
    let i2c: cap I2CBus<0x48> = I2CBus::new(0x48);
    let cooler: cap PWMPin<4> = PWMPin::new(4);

    let leitura: Result<f32, FalhaSensor> = self::monitorar(lim, i2c, cooler);
    match leitura {
        Ok(temp) => {
            io::print_f32("[RustTS v2.1.0-rts] Temperatura Nominal (C): ", temp);
            return 0;
        },
        Err(falha) => {
            io::println("[RustTS v2.1.0-rts] Alerta: Falha de Sensor Detectada!");
            return 1;
        },
    }
}
"#;
    let main_rts_path = src_dir.join("main.rts");
    if !main_rts_path.exists() {
        write_utf8_no_bom(&main_rts_path, main_rts);
    }

    println!(
        "[rtsc 2.1.0-rts] Initialized RustTS project in '{}'\n  -> Run project: rtsc run {}",
        target_dir.display(),
        main_rts_path.display()
    );
}

fn main() {
    let cli = Cli::parse();

    if cli.export_gbnf {
        println!("{}", export_rustts_gbnf());
        return;
    }

    match cli.command {
        Some(Commands::Init { path }) => {
            scaffold_project(&path);
        }
        Some(Commands::Check {
            file,
            json_diagnostics,
        }) => {
            let file_str = file.to_string_lossy().replace('\\', "/");
            if let Err(err) = fs::metadata(&file) {
                eprintln!("Failed to read '{}': {}", file_str, err);
                process::exit(1);
            }

            let (_module, report) = compile_and_check_project(&file);
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
            output,
            out_dir,
            compile_bin,
            emit_ts_sdk,
            emit_header,
            debug_symbols,
            json_diagnostics,
            link_libs,
            lib_dirs,
            include_dirs,
            fuel_limit,
        }) => {
            let file_str = file.to_string_lossy().replace('\\', "/");
            if let Err(err) = fs::metadata(&file) {
                eprintln!("Failed to read '{}': {}", file_str, err);
                process::exit(1);
            }

            let (module_opt, report) = compile_and_check_project(&file);
            if report.status != "ok" {
                if json_diagnostics {
                    println!("{}", report.to_json_pretty());
                } else {
                    eprintln!("{}", report.to_human_terminal());
                }
                process::exit(1);
            }

            let module = module_opt.expect("Valid module AST");
            let stem = file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("module");

            if emit_header {
                let header_code = NativeCodegen::emit_c_header(&module);
                if let Some(dir) = &out_dir {
                    let h_path = dir.join(format!("{}.h", stem));
                    write_utf8_no_bom(&h_path, &header_code);
                    println!("[rtsc 2.1.0-rts] Emitted C/C++ ABI header: {}", h_path.display());
                } else if let Some(out_path) = &output {
                    let h_path = out_path.with_extension("h");
                    write_utf8_no_bom(&h_path, &header_code);
                    println!("[rtsc 2.1.0-rts] Emitted C/C++ ABI header: {}", h_path.display());
                } else {
                    let h_path = PathBuf::from("build").join(format!("{}.h", stem));
                    write_utf8_no_bom(&h_path, &header_code);
                    println!("[rtsc 2.1.0-rts] Emitted C/C++ ABI header: {}", h_path.display());
                }
            }

            if emit_ts_sdk || target == "ts-sdk" {
                let ts_code = TsSdkCodegen::emit_typescript_sdk(&module);
                if let Some(out_path) = output {
                    write_utf8_no_bom(&out_path, &ts_code);
                    println!("[rtsc 2.1.0-rts] Emitted TypeScript SDK: {}", out_path.display());
                } else if let Some(dir) = out_dir {
                    let p = dir.join(format!("{}_sdk.ts", stem));
                    write_utf8_no_bom(&p, &ts_code);
                    println!("[rtsc 2.1.0-rts] Emitted TypeScript SDK: {}", p.display());
                } else {
                    println!("{}", ts_code);
                }
                return;
            }

            match target.as_str() {
                "native" | "c99" => {
                    let c99_code = NativeCodegen::emit_c99(&module);
                    let wants_bin = compile_bin
                        || output
                            .as_ref()
                            .and_then(|p| p.extension())
                            .and_then(|e| e.to_str())
                            .map(|ext| ext.eq_ignore_ascii_case("exe") || ext == "out")
                            .unwrap_or(false);

                    if wants_bin {
                        let build_dir = out_dir.unwrap_or_else(|| PathBuf::from("build"));
                        let c_path = build_dir.join(format!("{}.c", stem));
                        write_utf8_no_bom(&c_path, &c99_code);
                        let bin_path = output.unwrap_or_else(|| {
                            if cfg!(windows) {
                                build_dir.join(format!("{}.exe", stem))
                            } else {
                                build_dir.join(stem)
                            }
                        });
                        compile_c99_with_clang(&c_path, &bin_path, &include_dirs, &lib_dirs, &link_libs, debug_symbols);
                        println!(
                            "[rtsc 2.1.0-rts] Compiled native AVX2 binary: {}",
                            bin_path.display()
                        );
                    } else if let Some(out_path) = output {
                        write_utf8_no_bom(&out_path, &c99_code);
                        println!("[rtsc 2.1.0-rts] Emitted C99 source: {}", out_path.display());
                    } else if let Some(dir) = out_dir {
                        let p = dir.join(format!("{}.c", stem));
                        write_utf8_no_bom(&p, &c99_code);
                        println!("[rtsc 2.1.0-rts] Emitted C99 source: {}", p.display());
                    } else {
                        println!("{}", c99_code);
                    }
                }
                "unreal" | "unreal-cpp" => {
                    let artifacts = UnrealCodegen::emit_unreal_actor(&module);
                    if let Some(dir) = out_dir.or_else(|| {
                        output
                            .as_ref()
                            .and_then(|p| p.parent().map(|par| par.to_path_buf()))
                    }) {
                        for artifact in artifacts {
                            let h_path = dir.join(&artifact.header_filename);
                            let cpp_path = dir.join(&artifact.source_filename);
                            write_utf8_no_bom(&h_path, &artifact.header_code);
                            write_utf8_no_bom(&cpp_path, &artifact.source_code);
                            println!(
                                "[rtsc 2.1.0-rts] Emitted Unreal UHT files: {}, {}",
                                h_path.display(),
                                cpp_path.display()
                            );
                        }
                    } else {
                        for artifact in artifacts {
                            println!(
                                "// === FILE: {} ===\n{}",
                                artifact.header_filename, artifact.header_code
                            );
                            println!(
                                "// === FILE: {} ===\n{}",
                                artifact.source_filename, artifact.source_code
                            );
                        }
                    }
                }
                "unity" | "unity-burst" => {
                    let out = UnityCodegen::emit_unity_bindings(&module);
                    if let Some(dir) = out_dir.or_else(|| {
                        output
                            .as_ref()
                            .and_then(|p| p.parent().map(|par| par.to_path_buf()))
                    }) {
                        let rs_path = dir.join(format!("{}_unity_plugin.rs", stem));
                        let cs_path = dir.join("RtscNativeBridge.cs");
                        write_utf8_no_bom(&rs_path, &out.rust_ffi_code);
                        write_utf8_no_bom(&cs_path, &out.csharp_wrapper_code);
                        println!(
                            "[rtsc 2.1.0-rts] Emitted Unity FFI files: {}, {}",
                            rs_path.display(),
                            cs_path.display()
                        );
                    } else {
                        println!("// === RUST C-ABI PLUGIN ===\n{}", out.rust_ffi_code);
                        println!("// === C# NATIVEARRAY WRAPPER ===\n{}", out.csharp_wrapper_code);
                    }
                }
                "mlir" => {
                    let mlir_code = MlirCodegen::emit_mlir(&module);
                    if let Some(out_path) = output {
                        write_utf8_no_bom(&out_path, &mlir_code);
                        println!("[rtsc 2.1.0-rts] Emitted MLIR SSA module: {}", out_path.display());
                    } else if let Some(dir) = out_dir {
                        let p = dir.join(format!("{}.mlir", stem));
                        write_utf8_no_bom(&p, &mlir_code);
                        println!("[rtsc 2.1.0-rts] Emitted MLIR SSA module: {}", p.display());
                    } else {
                        println!("{}", mlir_code);
                    }
                }
                "llvm-ir" | "llvm" => {
                    let ir_code = LlvmIrCodegen::emit_llvm_ir(&module);
                    if compile_bin {
                        let build_dir = out_dir.unwrap_or_else(|| PathBuf::from("build"));
                        let ll_path = build_dir.join(format!("{}.ll", stem));
                        let bin_path = output.unwrap_or_else(|| {
                            if cfg!(windows) {
                                build_dir.join(format!("{}.exe", stem))
                            } else {
                                build_dir.join(stem)
                            }
                        });
                        write_utf8_no_bom(&ll_path, &ir_code);
                        compile_llvm_ir_with_clang(
                            &ll_path,
                            &bin_path,
                            &lib_dirs,
                            &link_libs,
                            debug_symbols,
                        );
                        println!(
                            "[rtsc 2.1.0-rts] Direct LLVM IR (.ll -> native binary) compiled: {}",
                            bin_path.display()
                        );
                    } else if let Some(out_path) = output {
                        write_utf8_no_bom(&out_path, &ir_code);
                        println!("[rtsc 2.1.0-rts] Emitted LLVM IR (.ll) module: {}", out_path.display());
                    } else if let Some(dir) = out_dir {
                        let p = dir.join(format!("{}.ll", stem));
                        write_utf8_no_bom(&p, &ir_code);
                        println!("[rtsc 2.1.0-rts] Emitted LLVM IR (.ll) module: {}", p.display());
                    } else {
                        println!("{}", ir_code);
                    }
                }
                "wgsl" => {
                    let wgsl_code = GpuCodegen::emit_wgsl(&module);
                    if let Some(out_path) = output {
                        write_utf8_no_bom(&out_path, &wgsl_code);
                        println!(
                            "[rtsc 2.1.0-rts] Emitted WebGPU WGSL Compute Shader: {}",
                            out_path.display()
                        );
                    } else if let Some(dir) = out_dir {
                        let p = dir.join(format!("{}.wgsl", stem));
                        write_utf8_no_bom(&p, &wgsl_code);
                        println!(
                            "[rtsc 2.1.0-rts] Emitted WebGPU WGSL Compute Shader: {}",
                            p.display()
                        );
                    } else {
                        println!("{}", wgsl_code);
                    }
                }
                "spirv" | "spv" => {
                    let want_binary = compile_bin
                        || target == "spv"
                        || output
                            .as_ref()
                            .and_then(|p| p.extension())
                            .and_then(|e| e.to_str())
                            == Some("spv");
                    if want_binary {
                        let spv_bytes = GpuCodegen::emit_spirv_binary(&module);
                        let out_path = output.unwrap_or_else(|| {
                            let dir = out_dir.unwrap_or_else(|| PathBuf::from("build"));
                            dir.join(format!("{}.spv", stem))
                        });
                        if let Some(parent) = out_path.parent() {
                            if !parent.as_os_str().is_empty() {
                                let _ = fs::create_dir_all(parent);
                            }
                        }
                        if let Err(err) = fs::write(&out_path, &spv_bytes) {
                            eprintln!("Failed to write '{}': {}", out_path.display(), err);
                            process::exit(1);
                        }
                        println!(
                            "[rtsc 2.1.0-rts] Emitted SPIR-V 1.5 Binary Module (magic 0x07230203, {} bytes): {}",
                            spv_bytes.len(),
                            out_path.display()
                        );
                    } else {
                        let spvasm_code = GpuCodegen::emit_spirv_asm(&module);
                        if let Some(out_path) = output {
                            write_utf8_no_bom(&out_path, &spvasm_code);
                            println!(
                                "[rtsc 2.1.0-rts] Emitted SPIR-V 1.5 Assembly (.spvasm): {}",
                                out_path.display()
                            );
                        } else if let Some(dir) = out_dir {
                            let p = dir.join(format!("{}.spvasm", stem));
                            write_utf8_no_bom(&p, &spvasm_code);
                            println!(
                                "[rtsc 2.1.0-rts] Emitted SPIR-V 1.5 Assembly (.spvasm): {}",
                                p.display()
                            );
                        } else {
                            println!("{}", spvasm_code);
                        }
                    }
                }
                "wasi" | "wat" | "wasm" => {
                    let want_wasm_bin = compile_bin
                        || target == "wasm"
                        || output
                            .as_ref()
                            .and_then(|p| p.extension())
                            .and_then(|e| e.to_str())
                            == Some("wasm");
                    if want_wasm_bin {
                        let wasm_bytes = WasiCodegen::emit_wasm_binary(&module, fuel_limit);
                        let out_path = output.unwrap_or_else(|| {
                            let dir = out_dir.unwrap_or_else(|| PathBuf::from("build"));
                            dir.join(format!("{}.wasm", stem))
                        });
                        if let Some(parent) = out_path.parent() {
                            if !parent.as_os_str().is_empty() {
                                let _ = fs::create_dir_all(parent);
                            }
                        }
                        if let Err(err) = fs::write(&out_path, &wasm_bytes) {
                            eprintln!("Failed to write '{}': {}", out_path.display(), err);
                            process::exit(1);
                        }
                        println!(
                            "[rtsc 2.1.0-rts] Emitted WASI Fuel-Metered Binary (.wasm, fuel={}, {} bytes): {}",
                            fuel_limit,
                            wasm_bytes.len(),
                            out_path.display()
                        );
                    } else {
                        let wat_code = WasiCodegen::emit_wat(&module, fuel_limit);
                        if let Some(out_path) = output {
                            write_utf8_no_bom(&out_path, &wat_code);
                            println!(
                                "[rtsc 2.1.0-rts] Emitted WASI Fuel-Metered Text (.wat, fuel={}): {}",
                                fuel_limit,
                                out_path.display()
                            );
                        } else if let Some(dir) = out_dir {
                            let p = dir.join(format!("{}.wat", stem));
                            write_utf8_no_bom(&p, &wat_code);
                            println!(
                                "[rtsc 2.1.0-rts] Emitted WASI Fuel-Metered Text (.wat, fuel={}): {}",
                                fuel_limit,
                                p.display()
                            );
                        } else {
                            println!("{}", wat_code);
                        }
                    }
                }
                other => {
                    eprintln!(
                        "Unsupported target '{}'. Valid targets: native, mlir, llvm-ir, wgsl, spirv, wasi, wasm, unreal, unity, ts-sdk",
                        other
                    );
                    process::exit(1);
                }
            }
        }
        Some(Commands::Run {
            file,
            llvm,
            debug_symbols,
            json_diagnostics,
            link_libs,
            lib_dirs,
            include_dirs,
            fuel_limit: _,
            run_args,
        }) => {
            let file_str = file.to_string_lossy().replace('\\', "/");
            if let Err(err) = fs::metadata(&file) {
                eprintln!("Failed to read '{}': {}", file_str, err);
                process::exit(1);
            }

            let (module_opt, report) = compile_and_check_project(&file);
            if report.status != "ok" {
                if json_diagnostics {
                    println!("{}", report.to_json_pretty());
                } else {
                    eprintln!("{}", report.to_human_terminal());
                }
                process::exit(1);
            }

            let module = module_opt.expect("Valid module AST");
            let stem = file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("app");
            let build_dir = PathBuf::from("build");
            let bin_path = if cfg!(windows) {
                build_dir.join(format!("{}.exe", stem))
            } else {
                build_dir.join(stem)
            };

            if llvm {
                let ir_code = LlvmIrCodegen::emit_llvm_ir(&module);
                let ll_path = build_dir.join(format!("{}.ll", stem));
                write_utf8_no_bom(&ll_path, &ir_code);
                compile_llvm_ir_with_clang(
                    &ll_path,
                    &bin_path,
                    &lib_dirs,
                    &link_libs,
                    debug_symbols,
                );
            } else {
                let c99_code = NativeCodegen::emit_c99(&module);
                let c_path = build_dir.join(format!("{}.c", stem));
                write_utf8_no_bom(&c_path, &c99_code);
                compile_c99_with_clang(
                    &c_path,
                    &bin_path,
                    &include_dirs,
                    &lib_dirs,
                    &link_libs,
                    debug_symbols,
                );
            }

            let run_status = Command::new(&bin_path)
                .args(&run_args)
                .status()
                .unwrap_or_else(|err| {
                    eprintln!("Failed to execute '{}': {}", bin_path.display(), err);
                    process::exit(1);
                });

            process::exit(run_status.code().unwrap_or(0));
        }
        None => {
            eprintln!("Usage: rtsc init [dir] | rtsc check <file.rts> | rtsc build <file.rts> [-o <out>] [--bin] | rtsc run <file.rts>");
            process::exit(1);
        }
    }
}
