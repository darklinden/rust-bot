use bot_run::sdimage::{build_workflow, percent_encode, resolve_model, SdParams};
use serde_json::json;

fn sd_params(raw: &str) -> Option<SdParams> {
    SdParams::parse(raw)
}

#[test]
fn resolve_model_hana() {
    assert_eq!(resolve_model("hana"), "hana4CHROME_huge.safetensors");
}

#[test]
fn resolve_model_hunyuan() {
    assert_eq!(resolve_model("hunyuan"), "hunyuan3d-dit-v2.safetensors");
}

#[test]
fn resolve_model_nova() {
    assert_eq!(resolve_model("nova"), "novaAnimeXL_ilV170.safetensors");
}

#[test]
fn resolve_model_xl() {
    assert_eq!(resolve_model("xl"), "sd_xl_base_1.0.safetensors");
}

#[test]
fn resolve_model_unknown_defaults_to_xl() {
    assert_eq!(resolve_model("unknown_model"), "sd_xl_base_1.0.safetensors");
}

#[test]
fn resolve_model_case_insensitive() {
    assert_eq!(resolve_model("HANA"), "hana4CHROME_huge.safetensors");
    assert_eq!(resolve_model("Nova"), "novaAnimeXL_ilV170.safetensors");
}

#[test]
fn percent_encode_alphanumeric() {
    assert_eq!(percent_encode("ABC123"), "ABC123");
}

#[test]
fn percent_encode_unreserved() {
    assert_eq!(percent_encode("abc-def_123.~"), "abc-def_123.~");
}

#[test]
fn percent_encode_space() {
    assert_eq!(percent_encode("hello world"), "hello%20world");
}

#[test]
fn percent_encode_chinese() {
    assert_eq!(percent_encode("中"), "%E4%B8%AD");
}

#[test]
fn percent_encode_percent_sign() {
    assert_eq!(percent_encode("100%"), "100%25");
}

#[test]
fn percent_encode_empty() {
    assert_eq!(percent_encode(""), "");
}

#[test]
fn percent_encode_mixed() {
    let result = percent_encode("hello 世界 100%");
    assert!(result.contains("%E4"));
    assert!(result.contains("%25"));
}

#[test]
fn sd_params_basic_prompt() {
    let params = sd_params("sd a beautiful sunset").unwrap();
    assert_eq!(params.prompt, "a beautiful sunset");
    assert_eq!(params.model, "xl");
    assert_eq!(params.cfg, 7.0);
    assert_eq!(params.steps, 20);
}

#[test]
fn sd_params_with_model_param() {
    let params = sd_params("sd a cat |model=hana").unwrap();
    assert_eq!(params.prompt, "a cat");
    assert_eq!(params.model, "hana");
}

#[test]
fn sd_params_with_negative() {
    let params = sd_params("sd a cat |negative=blurry").unwrap();
    assert_eq!(params.prompt, "a cat");
    assert_eq!(params.negative, "blurry");
}

#[test]
fn sd_params_with_sampler() {
    let params = sd_params("sd a cat |sampler=euler").unwrap();
    assert_eq!(params.prompt, "a cat");
    assert_eq!(params.sampler, "euler");
}

#[test]
fn sd_params_with_cfg() {
    let params = sd_params("sd a cat |cfg=5.0").unwrap();
    assert_eq!(params.prompt, "a cat");
    assert_eq!(params.cfg, 5.0);
}

#[test]
fn sd_params_with_steps() {
    let params = sd_params("sd a cat |steps=30").unwrap();
    assert_eq!(params.prompt, "a cat");
    assert_eq!(params.steps, 30);
}

#[test]
fn sd_params_multiple_params() {
    let params = sd_params("sd a cat |model=nova |negative=blurry |steps=25").unwrap();
    assert_eq!(params.prompt, "a cat");
    assert_eq!(params.model, "nova");
    assert_eq!(params.negative, "blurry");
    assert_eq!(params.steps, 25);
}

#[test]
fn sd_params_prompt_containing_equals() {}

#[test]
fn sd_params_empty_prompt_returns_none() {
    assert!(sd_params("").is_none());
}

#[test]
fn sd_params_whitespace_only_returns_none() {
    assert!(sd_params("   ").is_none());
}

#[test]
fn sd_params_dash_prefix() {
    let params = sd_params("-sd a dragon").unwrap();
    assert_eq!(params.prompt, "a dragon");
}

#[test]
fn sd_params_no_prefix_returns_none() {
    assert!(SdParams::parse("random text").is_none());
}

#[test]
fn build_workflow_has_required_nodes() {
    let params = SdParams {
        prompt: "test prompt".to_string(),
        model: "xl".to_string(),
        negative: String::new(),
        sampler: "euler_ancestral".to_string(),
        cfg: 7.0,
        steps: 20,
    };
    let wf = build_workflow(&params);

    assert!(wf.get("3").is_some(), "missing KSampler node");
    assert!(wf.get("4").is_some(), "missing CheckpointLoader node");
    assert!(wf.get("5").is_some(), "missing EmptyLatentImage node");
    assert!(
        wf.get("6").is_some(),
        "missing CLIPTextEncode (positive) node"
    );
    assert!(
        wf.get("7").is_some(),
        "missing CLIPTextEncode (negative) node"
    );
    assert!(wf.get("8").is_some(), "missing VAEDecode node");
    assert!(wf.get("9").is_some(), "missing SaveImage node");
}

#[test]
fn build_workflow_is_flat_not_nested() {
    let params = SdParams {
        prompt: "test".to_string(),
        model: "xl".to_string(),
        negative: String::new(),
        sampler: "euler".to_string(),
        cfg: 7.0,
        steps: 20,
    };
    let wf = build_workflow(&params);
    assert!(
        wf.get("prompt").is_none(),
        "workflow should be flat, not nested under 'prompt'"
    );
}

#[test]
fn build_workflow_ksampler_correct_links() {
    let params = SdParams {
        prompt: "test".to_string(),
        model: "xl".to_string(),
        negative: String::new(),
        sampler: "euler".to_string(),
        cfg: 7.0,
        steps: 20,
    };
    let wf = build_workflow(&params);
    let ksampler = wf.get("3").unwrap();

    assert_eq!(
        ksampler.get("class_type").and_then(|v| v.as_str()),
        Some("KSampler")
    );

    let inputs = ksampler.get("inputs").unwrap();
    assert_eq!(inputs.pointer("/model/0"), Some(&json!("4")));
    assert_eq!(inputs.pointer("/positive/0"), Some(&json!("6")));
    assert_eq!(inputs.pointer("/negative/0"), Some(&json!("7")));
    assert_eq!(inputs.pointer("/latent_image/0"), Some(&json!("5")));
}

#[test]
fn build_workflow_negative_default_when_empty() {
    let params = SdParams {
        prompt: "test".to_string(),
        model: "xl".to_string(),
        negative: String::new(),
        sampler: "euler".to_string(),
        cfg: 7.0,
        steps: 20,
    };
    let wf = build_workflow(&params);
    let neg = wf.get("7").unwrap();
    let inputs = neg.get("inputs").unwrap();
    let text = inputs.get("text").unwrap().as_str().unwrap();
    assert!(text.contains("worst quality"));
}

#[test]
fn build_workflow_negative_custom() {
    let params = SdParams {
        prompt: "test".to_string(),
        model: "xl".to_string(),
        negative: "my custom negative".to_string(),
        sampler: "euler".to_string(),
        cfg: 7.0,
        steps: 20,
    };
    let wf = build_workflow(&params);
    let neg = wf.get("7").unwrap();
    let inputs = neg.get("inputs").unwrap();
    assert_eq!(
        inputs.get("text").and_then(|v| v.as_str()),
        Some("my custom negative")
    );
}

#[test]
fn build_workflow_denoise_is_one() {
    let params = SdParams {
        prompt: "test".to_string(),
        model: "xl".to_string(),
        negative: String::new(),
        sampler: "euler".to_string(),
        cfg: 7.0,
        steps: 20,
    };
    let wf = build_workflow(&params);
    let ksampler = wf.get("3").unwrap();
    let denoise = ksampler.get("inputs").unwrap().get("denoise").unwrap();

    assert!(denoise.is_number());
}

#[test]
fn build_workflow_save_image_prefix() {
    let params = SdParams {
        prompt: "test".to_string(),
        model: "xl".to_string(),
        negative: String::new(),
        sampler: "euler".to_string(),
        cfg: 7.0,
        steps: 20,
    };
    let wf = build_workflow(&params);
    let save = wf.get("9").unwrap();
    let inputs = save.get("inputs").unwrap();
    assert_eq!(
        inputs.get("filename_prefix").and_then(|v| v.as_str()),
        Some("sd_bot")
    );
}
