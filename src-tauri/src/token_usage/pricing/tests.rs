use super::*;

#[test]
fn uses_july_gpt_5_6_variant_pricing() {
    for (model, input, cached, output) in [
        ("gpt-5.6-sol", 5.0, 0.5, 30.0),
        ("gpt-5.6-terra", 2.0, 0.2, 12.0),
        ("gpt-5.6-luna", 0.2, 0.02, 1.2),
        ("gpt-5.6", 5.0, 0.5, 30.0),
        ("gpt5.6-terra", 2.0, 0.2, 12.0),
        ("gpt-5-6-luna", 0.2, 0.02, 1.2),
        ("gpt-5.6-sol-2026-07-01", 5.0, 0.5, 30.0),
    ] {
        let rate =
            pricing_rate_for_model_at(model, GPT_5_6_TERRA_LUNA_PRICE_REDUCTION_EFFECTIVE_AT);
        assert_eq!(rate.input_per_million, input, "input price for {model}");
        assert_eq!(
            rate.cached_input_per_million, cached,
            "cached input price for {model}"
        );
        assert_eq!(rate.output_per_million, output, "output price for {model}");
    }
}

#[test]
fn prices_sol_cutover_and_astra_without_fallback_rates() {
    let usage = CodexTokenTotals {
        input_tokens: 1_000_000,
        cached_input_tokens: 500_000,
        output_tokens: 1_000_000,
        ..Default::default()
    };
    let cutoff = GPT_5_6_SOL_PRICE_REDUCTION_EFFECTIVE_AT;
    for model in [
        "gpt-5.6-sol",
        "gpt-5.6",
        "gpt5.6",
        "gpt-5-6",
        "gpt-5.6-sol-2026-07-01",
    ] {
        assert_eq!(estimate_token_cost_usd(model, cutoff - 1, &usage), 32.75);
        assert_eq!(estimate_token_cost_usd(model, cutoff, &usage), 22.2);
    }
    for model in [
        "gpt-6-astra",
        "GPT-6-ASTRA",
        "gpt-6",
        "gpt6",
        "gpt6-astra",
        "gpt-6-astra-2026-09-03",
        "gpt6-astra-2026-09-03",
    ] {
        assert_eq!(estimate_token_cost_usd(model, cutoff, &usage), 55.5);
    }
    assert_eq!(estimate_token_cost_usd("gpt-6-sol", cutoff, &usage), 11.1);
    assert_eq!(
        estimate_token_cost_usd("gpt6-sol-2026-09-22", cutoff, &usage),
        11.1
    );
    assert_eq!(estimate_token_cost_usd("gpt-6-luna", cutoff, &usage), 0.555);
    assert_eq!(
        estimate_token_cost_usd("gpt6-luna-2026-09-22", cutoff, &usage),
        0.555
    );
}

#[test]
fn preserves_pre_reduction_gpt_5_6_terra_and_luna_pricing() {
    for (model, input, cached, output) in [
        ("gpt-5.6-terra", 2.5, 0.25, 15.0),
        ("gpt5.6-terra", 2.5, 0.25, 15.0),
        ("gpt-5.6-luna", 1.0, 0.1, 6.0),
        ("gpt-5-6-luna", 1.0, 0.1, 6.0),
    ] {
        let rate =
            pricing_rate_for_model_at(model, GPT_5_6_TERRA_LUNA_PRICE_REDUCTION_EFFECTIVE_AT - 1);
        assert_eq!(rate.input_per_million, input, "input price for {model}");
        assert_eq!(
            rate.cached_input_per_million, cached,
            "cached input price for {model}"
        );
        assert_eq!(rate.output_per_million, output, "output price for {model}");
    }
}

#[test]
fn prices_mixed_gpt_5_6_history_by_event_timestamp() {
    let usage = CodexTokenTotals {
        input_tokens: 1_000_000,
        cached_input_tokens: 0,
        output_tokens: 1_000_000,
        reasoning_output_tokens: 0,
        total_tokens: 2_000_000,
    };
    let before = GPT_5_6_TERRA_LUNA_PRICE_REDUCTION_EFFECTIVE_AT - 1;
    let after = GPT_5_6_TERRA_LUNA_PRICE_REDUCTION_EFFECTIVE_AT;

    assert_eq!(
        estimate_token_cost_usd("gpt-5.6-terra", before, &usage),
        17.5
    );
    assert_eq!(
        estimate_token_cost_usd("gpt-5.6-terra", after, &usage),
        14.0
    );
    assert_eq!(estimate_token_cost_usd("gpt-5.6-luna", before, &usage), 7.0);
    assert_eq!(estimate_token_cost_usd("gpt-5.6-luna", after, &usage), 1.4);
}
