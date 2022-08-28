#![feature(fs_try_exists, io_read_to_string)]
use md5::{Digest, Md5};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    str,
};
use which::which;

const TOOLS_URL: &str = "https://download.clojure.org/install/clojure-tools-1.11.1.1113.zip";

const VERSION: &str = "1.11.1.1113";

#[derive(Debug)]
pub enum ExecOpts {
    // Default
    Repl,
    // -Aaliases      Use concatenated aliases to modify classpath
    Alias(String),
    // -X[aliases]    Use concatenated aliases to modify classpath or supply exec fn/args
    Exec(String),
    // -T[name|aliases]  Invoke tool by name or via aliases ala -X
    Tool(String),
    // -M[aliases]    Use concatenated aliases to modify classpath or supply main opts
    Main(String),
    // -P             Prepare deps - download libs, cache classpath, but don't exec
    Prepare,
}

#[derive(Debug, Default)]
pub struct CljOpts {
    repl_aliases: Vec<String>,
    /// -Jopt Pass opt through in java_opts, ex: -J-Xmx512m
    jvm_opts: String,
    // -Sdeps EDN Deps data to use as the final deps file
    //deps: String,
    //cp: String, // force cp
    /// -Spath         Compute classpath and echo to stdout only
    path: bool,
    /// -Spom          Generate (or update) pom.xml with deps and paths
    pom: bool,
    /// -Stree         Print dependency tree
    tree: bool,
    verbose: bool,
    /// remain clojure args
    clojure_args: Vec<String>,
}

fn get_java_command() -> anyhow::Result<PathBuf> {
    if let Ok(java) = which("java") {
        return Ok(java);
    }

    if let Ok(java_home) = env::var("JAVA_HOME") {
        let java_home = PathBuf::from(java_home);
        for candidate in &["bin/java.exe", "bin/java"] {
            let java = java_home.join(candidate);
            if java.exists() {
                return Ok(dunce::canonicalize(java)?);
            }
        }
    }
    anyhow::bail!("Couldn't find 'java'. Please set JAVA_HOME.")
}

/// Determine user config directory
fn get_clj_config() -> anyhow::Result<PathBuf> {
    env::var("CLJ_CONFIG")
        .or_else(|_| env::var("HOME").map(|s| s + "/.clojure"))
        .or_else(|_| env::var("USERPROFILE").map(|s| s + "/.clojure"))
        .map(PathBuf::from)
        .map_err(anyhow::Error::from)
}

/// Determine user cache directory
fn get_clj_cache() -> anyhow::Result<PathBuf> {
    env::var("CLJ_CACHE")
        .map(PathBuf::from)
        .map_err(anyhow::Error::from)
        .or_else(|_| get_clj_config().map(|s| s.join(".cpcache")))
}

fn parse_args() -> Option<(ExecOpts, CljOpts)> {
    let args = env::args().collect::<Vec<_>>();

    // println!("args => {:?}", args);
    let mut exec_opts = ExecOpts::Repl;
    let mut clj_opts = CljOpts::default();

    let mut it = args.into_iter().skip(1);

    while let Some(arg) = it.next() {
        if arg == "-version" || arg == "--version" {
            println!("Clojure CLI version {}", VERSION);
            return None;
        } else if arg.starts_with("-J") {
            if clj_opts.jvm_opts.is_empty() {
                clj_opts.jvm_opts = arg[2..].to_owned();
            } else {
                clj_opts.jvm_opts.push(' ');
                clj_opts.jvm_opts.push_str(&arg[2..]);
            }
        } else if arg == "-M" {
            exec_opts = ExecOpts::Main("".to_owned());
            clj_opts.clojure_args.extend(it);
            break;
        } else if arg == "-M:" {
            exec_opts = ExecOpts::Main(it.next().unwrap());
            clj_opts.clojure_args.extend(it);
            break;
        } else if arg == "-T" {
            exec_opts = ExecOpts::Tool("".to_owned());
            clj_opts.clojure_args.extend(it);
            break;
        } else if arg == "-T:" {
            exec_opts = ExecOpts::Tool(it.next().unwrap());
            clj_opts.clojure_args.extend(it);
            break;
        } else if arg.starts_with("-T") {
            exec_opts = ExecOpts::Tool(arg[2..].to_owned());
            clj_opts.clojure_args.extend(it);
            break;
        } else if arg.starts_with("-A") {
            // repl alias
            clj_opts.repl_aliases.push(arg[2..].to_owned());
        } else if arg == "-X" {
            exec_opts = ExecOpts::Exec("".to_owned());
            clj_opts.clojure_args.extend(it);
            break;
        } else if arg == "-X:" {
            exec_opts = ExecOpts::Exec(it.next().unwrap());
            clj_opts.clojure_args.extend(it);
            break;
        } else if arg == "-P" {
            exec_opts = ExecOpts::Prepare;
            clj_opts.clojure_args.extend(it);
            break;
        } else if arg == "-Sdeps" {
            unimplemented!()
        } else if arg == "-Scp" {
            unimplemented!()
        } else if arg == "-Spath" {
            clj_opts.path = true;
        } else if arg == "-Sverbose" {
            clj_opts.verbose = true;
        } else if arg == "-Stree" {
            clj_opts.tree = true;
        } else if arg == "-Spom" {
            clj_opts.pom = true;
        } else if arg == "-h" || arg == "--help" || arg == "-?" {
            if let ExecOpts::Main(_) = exec_opts {
                clj_opts.clojure_args.push(arg);
                clj_opts.clojure_args.extend(it);
                break;
            } else {
                unimplemented!("help");
            }
        } else if arg.starts_with("-S") {
            panic!("unsupported option: {}", arg);
        } else if arg == "--" {
            clj_opts.clojure_args.extend(it);
            break;
        } else {
            clj_opts.clojure_args.push(arg);
            clj_opts.clojure_args.extend(it);
            break;
        }
    }

    Some((exec_opts, clj_opts))
}

fn md5_string(s: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn main() -> anyhow::Result<()> {
    let (exec_opts, clj_opts) = match parse_args() {
        Some(v) => v,
        None => return Ok(()),
    };
    // println!("D: runing: {:?}", exec_opts);

    let java = get_java_command()?;

    let install_dir =
        PathBuf::from(r#"C:\Windows\system32\WindowsPowerShell\v1.0\Modules\ClojureTools\"#);

    let config_dir = get_clj_config()?;
    // If user config directory does not exist, create it
    fs::create_dir_all(&config_dir)?;

    if !config_dir.join("deps.edn").exists() {
        fs::copy(
            install_dir.join("example-deps.edn"),
            config_dir.join("deps.edn"),
        )?;
    }
    if !config_dir.join("tools").exists() {
        fs::create_dir_all(config_dir.join("tools"))?;
    }
    if install_dir.join("tools.edn").metadata()?.modified()?
        > config_dir.join("tools/tools.edn").metadata()?.modified()?
    {
        fs::copy(
            install_dir.join("tools.edn"),
            config_dir.join("tools/tools.edn"),
        )?;
    }

    let user_cache_dir = get_clj_cache()?;
    if clj_opts.verbose {
        println!("D: java {}", java.display());
        println!("D: config_dir {}", config_dir.display());
        println!("D: user_cache_dir {}", user_cache_dir.display());
    }

    // Chain deps.edn in config paths. repro=skip config dir
    let project_config = "deps.edn";
    // TODO: handle Repro options
    let user_config = config_dir.join("deps.end");
    let config_paths = &[
        install_dir.join("deps.edn"),
        config_dir.join("deps.edn"),
        "deps.edn".into(),
    ];

    // Determine whether to use user or project cache
    let cache_dir = if Path::new("deps.edn").exists() {
        ".cpcache".into()
    } else {
        user_cache_dir.clone()
    };

    // Construct location of cached classpath file
    let cache_key = format!(
        "|{:?}|{:?}|{:?}|",
        &exec_opts, clj_opts.clojure_args, config_paths
    );
    let cache_key_hash = md5_string(&cache_key);
    if clj_opts.verbose {
        println!("D: cache key: {}", cache_key_hash);
    }

    let libs_file = cache_dir.join(cache_key_hash.to_owned() + ".libs");
    let cp_file = cache_dir.join(cache_key_hash.to_owned() + ".cp");
    let jvm_file = cache_dir.join(cache_key_hash.to_owned() + ".jvm");
    let main_file = cache_dir.join(cache_key_hash.to_owned() + ".main");
    let basis_file = cache_dir.join(cache_key_hash.to_owned() + ".basis");
    let manifest_file = cache_dir.join(cache_key_hash.to_owned() + ".manifest");

    if clj_opts.verbose {
        println!("version      {}", VERSION);
        println!("install_dir  {}", install_dir.display());
        println!("config_dir   {}", config_dir.display());
        println!("config_paths {:?}", config_paths);
        println!("cache_dir    {}", cache_dir.display());
        println!("cp_file      {}", cp_file.display());
    }

    // Make tools args if needed
    let mut tools_args: Vec<String> = vec![];
    if let ExecOpts::Main(alias) = &exec_opts {
        if !alias.is_empty() {
            tools_args.push(format!("-M{}", alias))
        }
    }
    if !clj_opts.repl_aliases.is_empty() {
        tools_args.push(clj_opts.repl_aliases.join(""));
    }
    if let ExecOpts::Exec(alias) = &exec_opts {
        if !alias.is_empty() {
            tools_args.push(format!("-X{}", alias))
        }
    }
    // tool mode, use tool name or tool alias
    if let ExecOpts::Tool(alias) = &exec_opts {
        tools_args.push("--tool-mode".into());
        if alias.is_empty() {
            unimplemented!()
        } else if alias.starts_with(":") {
            tools_args.push(format!("-T{}", alias))
        } else {
            tools_args.push("--tool-name".into());
            tools_args.push(alias.into());
        }
    }
    if clj_opts.tree {
        tools_args.push("--tree".into());
    }

    // If stale, run make-classpath to refresh cached classpath
    if clj_opts.verbose {
        println!("Refreshing classpath");
    }
    let tools_cp = r#"C:\Windows\system32\WindowsPowerShell\v1.0\Modules\ClojureTools\clojure-tools-1.11.1.1113.jar"#;

    let child = Command::new(&java)
        .args([
            "-classpath",
            tools_cp,
            "clojure.main",
            "-m",
            "clojure.tools.deps.alpha.script.make-classpath2",
        ])
        .arg("--config-user")
        .arg(user_config.as_os_str())
        .arg("--config-project")
        .arg(project_config)
        .arg("--basis-file")
        .arg(basis_file.as_os_str())
        .arg("--libs-file")
        .arg(libs_file.as_os_str())
        .arg("--cp-file")
        .arg(cp_file.as_os_str())
        .arg("--jvm-file")
        .arg(jvm_file.as_os_str())
        .arg("--main-file")
        .arg(main_file.as_os_str())
        .arg("--manifest-file")
        .arg(manifest_file.as_os_str())
        .args(tools_args)
        .spawn()
        .expect("run");
    let output = child.wait_with_output()?;
    if !output.status.success() {
        anyhow::bail!("refresh classpath: {}", output.status);
    }

    // println!("stdout: {}", std::str::from_utf8(&output.stdout)?);
    // println!("stderr: {}", std::str::from_utf8(&output.stderr)?);

    let cp = fs::read_to_string(cp_file)?;
    if clj_opts.verbose {
        println!("D class path: {}", cp);
    }

    let jvm_cache_opts = if jvm_file.exists() {
        fs::read_to_string(jvm_file)?
    } else {
        Default::default()
    };

    let maybe_child = match exec_opts {
        ExecOpts::Exec(_) | ExecOpts::Tool(_) => Command::new(&java)
            .args(jvm_cache_opts.split_whitespace().collect::<Vec<_>>())
            .args(clj_opts.jvm_opts.split_whitespace().collect::<Vec<_>>())
            .arg(format!("-Dclojure.basis={}", basis_file.display()))
            .arg("-classpath")
            .arg(format!("{};{}/exec.jar", cp, install_dir.display()))
            .arg("clojure.main")
            .arg("-m")
            .arg("clojure.run.exec")
            .args(&clj_opts.clojure_args)
            .spawn(),
        ExecOpts::Main(_) | ExecOpts::Alias(_) | ExecOpts::Repl => {
            let main_cache_opts = if main_file.exists() {
                fs::read_to_string(main_file)?
            } else {
                Default::default()
            };
            Command::new(&java)
                .args(jvm_cache_opts.split_whitespace().collect::<Vec<_>>())
                .args(clj_opts.jvm_opts.split_whitespace().collect::<Vec<_>>())
                .arg(format!("-Dclojure.basis={}", basis_file.display()))
                .arg("-classpath")
                .arg(cp)
                .arg("clojure.main")
                .args(main_cache_opts.split_ascii_whitespace().collect::<Vec<_>>())
                .args(&clj_opts.clojure_args)
                .spawn()
        }
        ExecOpts::Prepare => {
            return Ok(());
        }
    };

    let mut child = maybe_child.expect("repl ok");
    child.wait()?;

    Ok(())
}
