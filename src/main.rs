#![feature(rustc_private)]

extern crate rustc_codegen_ssa;
extern crate rustc_errors;
extern crate rustc_hash;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_metadata;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use rustc_hir::def::DefKind;
use rustc_middle::ty::Instance;
use rustc_session::{
    config::{CodegenOptions, Input, Options},
    DiagnosticOutput,
};
use rustc_span::FileName;

use anyhow::{Context, Result};
use reedline::{
    default_emacs_keybindings, DefaultValidator, EditCommand, Emacs, KeyModifiers, Prompt,
    Reedline, Signal,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;

use std::borrow::Cow;
use std::ffi::CString;

mod backend;

struct IrsPrompt;

impl Prompt for IrsPrompt {
    fn render_prompt_left(&self) -> Cow<str> {
        Cow::Borrowed("")
    }

    fn render_prompt_right(&self) -> Cow<str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _prompt_mode: reedline::PromptEditMode) -> Cow<str> {
        Cow::Borrowed("irs> ")
    }

    fn render_prompt_multiline_indicator(&self) -> std::borrow::Cow<str> {
        Cow::Borrowed(".... ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        _history_search: reedline::PromptHistorySearch,
    ) -> std::borrow::Cow<str> {
        todo!()
    }

    fn get_indicator_color(&self) -> crossterm::style::Color {
        crossterm::style::Color::Cyan
    }
}

fn config(sysroot: &str, code: String) -> rustc_interface::Config {
    rustc_interface::Config {
        opts: Options {
            maybe_sysroot: Some(std::path::PathBuf::from(sysroot)),
            cg: CodegenOptions {
                overflow_checks: Some(false),
                ..Default::default()
            },
            ..Default::default()
        },
        crate_cfg: Default::default(),
        crate_check_cfg: Default::default(),
        input: Input::Str {
            name: FileName::Custom("IRS".to_owned()),
            input: code,
        },
        input_path: None,
        output_file: None,
        output_dir: None,
        file_loader: None,
        diagnostic_output: DiagnosticOutput::Default,
        lint_caps: Default::default(),
        parse_sess_created: None,
        register_lints: None,
        override_queries: None,
        make_codegen_backend: Some(Box::new(|_| Box::new(backend::DummyBackend))),
        registry: rustc_errors::registry::Registry::new(&[]),
    }
}

fn main() -> Result<()> {
    tracing_subscriber::Registry::default()
        .with(EnvFilter::from_default_env())
        .with(
            HierarchicalLayer::new(2)
                .with_indent_lines(true)
                .with_bracketed_fields(true),
        )
        .init();

    let sysroot = std::process::Command::new("rustc")
        .arg("--print=sysroot")
        .output()
        .expect("Failed to run `rustc --print=sysroot`");
    let sysroot = std::str::from_utf8(&sysroot.stdout).unwrap().trim();

    let mut keybindings = default_emacs_keybindings();
    keybindings.add_binding(
        KeyModifiers::NONE,
        reedline::KeyCode::Tab,
        reedline::ReedlineEvent::Edit(vec![EditCommand::InsertString("   ".to_owned())]),
    );

    let edit_mode = Emacs::new(keybindings);
    let validator = Box::new(DefaultValidator);
    let mut line_editor = Reedline::create()
        .with_edit_mode(Box::new(edit_mode))
        .with_validator(validator);
    let prompt = IrsPrompt;
    loop {
        let code = match line_editor
            .read_line(&prompt)
            .context("Failed to read line")?
        {
            Signal::Success(buffer) => buffer,
            Signal::CtrlC | Signal::CtrlD => break Ok(()),
        };

        let config = config(sysroot, code);

        let now = std::time::Instant::now();
        rustc_interface::run_compiler(config, |compiler| {
            compiler.enter(|queries| {
                queries.global_ctxt().unwrap().take().enter(|tcx| unsafe {
                    // let llvm_module = rustjitc::ModuleLlvm::new(tcx, "test");
                    // let cx = rustjitc::context::CodegenCx::new(tcx, &llvm_module);
                    if let Some(func) = tcx
                        .hir_crate_items(())
                        .items()
                        .find(|item| tcx.def_kind(item.def_id) == DefKind::Fn)
                    {
                        rustjitc::eval_func(tcx, func.def_id.to_def_id());
                        // let instance = Instance::mono(tcx, func.def_id.to_def_id());

                        // rustjitc::mir::codegen_func(&cx, func.def_id.to_def_id());
                        // LLVMLinkInMCJIT();

                        // let mut ee = std::mem::MaybeUninit::uninit();
                        // let mut zero = std::mem::MaybeUninit::zeroed();

                        // LLVMCreateExecutionEngineForModule(ee.as_mut_ptr(), llvm_module.llmod, zero.as_mut_ptr());
                        // let addr = LLVMGetFunctionAddress(ee.assume_init(), tcx.symbol_name(instance).name.as_ptr().cast());

                        // let f: extern "C" fn() -> i32 = std::mem::transmute(addr);
                        // dbg!(f());

                        // LLVMDisposeExecutionEngine(ee.assume_init());
                    }
                })
            })
        });

        println!("{}ms have passed", now.elapsed().as_millis());
    }
}
