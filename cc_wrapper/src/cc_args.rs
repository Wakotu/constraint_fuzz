use color_eyre::eyre::Result;
use std::{env, path::Path};
use which;

use colored::Colorize;

fn is_path_exe(cmd: &str) -> bool {
    let res = which::which(cmd);
    res.is_ok()
}

struct ClState {
    link: bool,
    cmpl: bool,
}

enum CcType {
    Cc,
    Cxx,
}

pub struct CCArgs {
    args: Vec<String>,
    cl_stat: ClState,
    cc_type: CcType,
}

impl CCArgs {
    fn get_cc_path_by_name(name: &str) -> Result<String> {
        let cl_path = which::which(name)?;
        let cl_str = cl_path.to_str().unwrap_or_else(|| {
            panic!("Failed to convert clang++ path to string");
        });
        Ok(cl_str.to_string())
    }

    fn init_check(args: &[String]) -> Result<()> {
        let cc = &args[0];
        assert!(
            is_path_exe(cc),
            "First argument passsed in is not an executable"
        );

        Ok(())
    }

    fn get_suffix(name: &str) -> Option<&str> {
        name.rfind('.').map(|dot_index| &name[dot_index + 1..])
    }

    fn is_opt(arg: &str) -> bool {
        arg.starts_with('-')
    }

    fn get_base_name(fpath: &str) -> &str {
        let path = Path::new(fpath);
        let name = path
            .file_name()
            .unwrap_or_else(|| panic!("Failed to get base name from path {:?}", path));

        let name = name
            .to_str()
            .unwrap_or_else(|| panic!("Failed to get string from basename {:?}", name));
        name
    }

    fn is_src_file(arg: &str) -> bool {
        if Self::is_opt(arg) {
            return false;
        }
        let suf_op = Self::get_suffix(arg);
        if suf_op.is_none() {
            return false;
        }

        let suf = suf_op.unwrap();
        if "c" == suf || "cc" == suf || "cpp" == suf {
            return true;
        }
        false
    }

    fn is_input_arg(arg: &str) -> bool {
        let suf_op = Self::get_suffix(arg);
        if suf_op.is_none() {
            return false;
        }
        let suf = suf_op.unwrap();
        "c" == suf || "cc" == suf || "cpp" == suf || "o" == suf || "so" == suf || "a" == suf
    }

    fn has_input_arg(args: &[String]) -> bool {
        for arg in args.iter() {
            if Self::is_input_arg(arg) {
                return true;
            }
        }
        false
    }

    fn get_cl_state(args: &[String]) -> ClState {
        if !Self::has_input_arg(args) {
            return ClState {
                link: false,
                cmpl: false,
            };
        }

        let mut link = true;
        let mut cmpl = false;

        for arg in args.iter() {
            if "-c" == arg {
                link = false;
            }

            if Self::is_src_file(arg) {
                cmpl = true;
            }
        }
        ClState { link, cmpl }
    }

    fn get_cc_type(args: &[String]) -> CcType {
        let cc_cmd = &args[0];
        let name = Self::get_base_name(cc_cmd);
        if "cc_wrapper" == name {
            CcType::Cc
        } else if "cxx_wrapper" == name {
            CcType::Cxx
        } else {
            panic!("Invalid cc name");
        }
    }

    fn contains_link(&self) -> bool {
        self.cl_stat.link
    }

    fn contains_cmpl(&self) -> bool {
        self.cl_stat.cmpl
    }

    pub fn from_cli() -> Result<Self> {
        let args: Vec<String> = env::args().collect();
        Self::init_check(&args)?;

        let cl_stat = Self::get_cl_state(&args);
        let cc_type = Self::get_cc_type(&args);

        Ok(Self {
            args,
            cl_stat,
            cc_type,
        })
    }

    pub fn append_arg(&mut self, arg: String) {
        self.args.push(arg);
    }

    fn cc_subst(&mut self) -> Result<()> {
        match self.cc_type {
            CcType::Cc => {
                self.args[0] = Self::get_cc_path_by_name("clang")?;
            }
            CcType::Cxx => {
                self.args[0] = Self::get_cc_path_by_name("clang++")?;
            }
        }
        Ok(())
    }

    fn add_plugin_flags(&mut self) -> Result<()> {
        if !self.contains_cmpl() {
            return Ok(());
        }

        let home = env::var("HOME")?;
        let plg_path = Path::new(&home)
            .join(".local")
            .join("lib")
            .join("func_seq_pass")
            .join("libfunc_seq_pass.so");

        // check
        assert!(
            plg_path.is_file(),
            "Pass Plugin file {:?} not found",
            plg_path
        );

        let plg_flag = format!("-fpass-plugin={}", plg_path.to_string_lossy());
        self.append_arg(plg_flag);

        Ok(())
    }

    fn add_link_flags(&mut self) -> Result<()> {
        if !self.contains_link() {
            return Ok(());
        }

        let home = env::var("HOME")?;
        let lib_dir = Path::new(&home)
            .join(".local")
            .join("lib")
            .join("func_seq_pass");
        let lib_dir_flag = format!("-L{}", lib_dir.to_string_lossy());
        self.append_arg(lib_dir_flag);

        let lib_name = "func_stack";
        let lib_flag = format!("-l{}", lib_name);
        self.append_arg(lib_flag);

        Ok(())
    }

    fn add_std_cxx_link_flag(&mut self) -> Result<()> {
        if !self.contains_link() {
            return Ok(());
        }

        if let CcType::Cxx = self.cc_type {
            return Ok(());
        }

        self.append_arg("-lstdc++".to_string());

        Ok(())
    }

    pub fn transform(&mut self) -> Result<()> {
        self.cc_subst()?;
        self.add_plugin_flags()?;
        self.add_link_flags()?;
        self.add_std_cxx_link_flag()?;
        Ok(())
    }

    fn show_args(&self) {
        print!("{}", "under invocation".blue().bold());
        for (idx, arg) in self.args.iter().enumerate() {
            if idx > 0 {
                print!(" ");
            }
            print!("{}", arg);
        }
        println!();
    }

    pub fn output(self) -> Vec<String> {
        self.show_args();
        self.args
    }
}
