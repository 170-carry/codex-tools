//! Standard API token price estimates selected at each usage event time.

use super::CodexTokenTotals;

pub(super) const PRICING_SOURCE: &str =
    "OpenAI API standard short-context pricing, historical rates by event time; GPT-6/GPT-5.6 checked 2026-09-29";
// OpenAI announced lower GPT-5.6 Terra and Luna prices effective 2026-07-30.
// The announcement only specifies the date, so analytics use the UTC day boundary.
// Source: https://developers.openai.com/api/docs/changelog
const GPT_5_6_TERRA_LUNA_PRICE_REDUCTION_EFFECTIVE_AT: i64 = 1_785_369_600;
// Sol price reduction announced 2026-08-21; use the UTC day boundary because
// the official changelog does not specify an intraday effective time.
// https://developers.openai.com/api/docs/changelog
const GPT_5_6_SOL_PRICE_REDUCTION_EFFECTIVE_AT: i64 = 1_787_270_400;

struct PricingRate {
    input_per_million: f64,
    cached_input_per_million: f64,
    output_per_million: f64,
}

pub(super) fn round_cost(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

pub(super) fn estimate_token_cost_usd(
    model: &str,
    event_timestamp: i64,
    usage: &CodexTokenTotals,
) -> f64 {
    let rate = pricing_rate_for_model_at(model, event_timestamp);
    let cached_input = usage.cached_input_tokens.min(usage.input_tokens);
    let uncached_input = usage.input_tokens.saturating_sub(cached_input);
    let cost = (uncached_input as f64 * rate.input_per_million
        + cached_input as f64 * rate.cached_input_per_million
        + usage.output_tokens as f64 * rate.output_per_million)
        / 1_000_000.0;
    round_cost(cost)
}

fn pricing_rate_for_model_at(model: &str, event_timestamp: i64) -> PricingRate {
    let normalized = model.to_ascii_lowercase();
    // Standard short-context rates, not subscription billing or fast-tier rates.
    // GPT-6 standard short-context rates from the official model pages.
    // https://developers.openai.com/api/docs/models/gpt-6-astra
    if matches!(
        normalized.as_str(),
        "gpt-6" | "gpt6" | "gpt-6-astra" | "gpt6-astra"
    ) || normalized.starts_with("gpt-6-astra-")
        || normalized.starts_with("gpt6-astra-")
    {
        return PricingRate {
            input_per_million: 10.0,
            cached_input_per_million: 1.0,
            output_per_million: 50.0,
        };
    }
    if normalized.starts_with("gpt-6-sol") || normalized.starts_with("gpt6-sol") {
        // https://developers.openai.com/api/docs/models/gpt-6-sol
        return PricingRate {
            input_per_million: 2.0,
            cached_input_per_million: 0.2,
            output_per_million: 10.0,
        };
    }
    if normalized.starts_with("gpt-6-luna") || normalized.starts_with("gpt6-luna") {
        // https://developers.openai.com/api/docs/models/gpt-6-luna
        return PricingRate {
            input_per_million: 0.1,
            cached_input_per_million: 0.01,
            output_per_million: 0.5,
        };
    }
    if normalized == "gpt-5.6"
        || normalized == "gpt5.6"
        || normalized == "gpt-5-6"
        || normalized.starts_with("gpt-5.6-sol")
        || normalized.starts_with("gpt5.6-sol")
        || normalized.starts_with("gpt-5-6-sol")
    {
        // https://developers.openai.com/api/docs/models/gpt-5.6-sol
        if event_timestamp >= GPT_5_6_SOL_PRICE_REDUCTION_EFFECTIVE_AT {
            return PricingRate {
                input_per_million: 4.0,
                cached_input_per_million: 0.4,
                output_per_million: 20.0,
            };
        }
        return PricingRate {
            input_per_million: 5.0,
            cached_input_per_million: 0.5,
            output_per_million: 30.0,
        };
    }
    if normalized.starts_with("gpt-5.6-terra")
        || normalized.starts_with("gpt5.6-terra")
        || normalized.starts_with("gpt-5-6-terra")
    {
        if event_timestamp >= GPT_5_6_TERRA_LUNA_PRICE_REDUCTION_EFFECTIVE_AT {
            return PricingRate {
                input_per_million: 2.0,
                cached_input_per_million: 0.2,
                output_per_million: 12.0,
            };
        }
        return PricingRate {
            input_per_million: 2.5,
            cached_input_per_million: 0.25,
            output_per_million: 15.0,
        };
    }
    if normalized.starts_with("gpt-5.6-luna")
        || normalized.starts_with("gpt5.6-luna")
        || normalized.starts_with("gpt-5-6-luna")
    {
        if event_timestamp >= GPT_5_6_TERRA_LUNA_PRICE_REDUCTION_EFFECTIVE_AT {
            return PricingRate {
                input_per_million: 0.2,
                cached_input_per_million: 0.02,
                output_per_million: 1.2,
            };
        }
        return PricingRate {
            input_per_million: 1.0,
            cached_input_per_million: 0.1,
            output_per_million: 6.0,
        };
    }
    if normalized.starts_with("gpt-5.5-pro") {
        return PricingRate {
            input_per_million: 15.0,
            cached_input_per_million: 15.0,
            output_per_million: 90.0,
        };
    }
    if normalized.starts_with("gpt-5.5") {
        return PricingRate {
            input_per_million: 2.5,
            cached_input_per_million: 0.25,
            output_per_million: 15.0,
        };
    }
    if normalized.starts_with("gpt-5.4-pro") {
        return PricingRate {
            input_per_million: 15.0,
            cached_input_per_million: 15.0,
            output_per_million: 90.0,
        };
    }
    if normalized.starts_with("gpt-5.4-mini") {
        return PricingRate {
            input_per_million: 0.375,
            cached_input_per_million: 0.0375,
            output_per_million: 2.25,
        };
    }
    if normalized.starts_with("gpt-5.4-nano") {
        return PricingRate {
            input_per_million: 0.1,
            cached_input_per_million: 0.01,
            output_per_million: 0.625,
        };
    }
    if normalized.starts_with("gpt-5.4") {
        return PricingRate {
            input_per_million: 1.25,
            cached_input_per_million: 0.13,
            output_per_million: 7.5,
        };
    }
    if normalized.contains("codex-mini") || normalized.starts_with("gpt-5-mini") {
        return PricingRate {
            input_per_million: 0.25,
            cached_input_per_million: 0.025,
            output_per_million: 2.0,
        };
    }
    if normalized.starts_with("gpt-5-nano") {
        return PricingRate {
            input_per_million: 0.05,
            cached_input_per_million: 0.005,
            output_per_million: 0.4,
        };
    }
    if normalized.starts_with("o4-mini") {
        return PricingRate {
            input_per_million: 1.1,
            cached_input_per_million: 0.275,
            output_per_million: 4.4,
        };
    }
    if normalized.starts_with("o3") {
        return PricingRate {
            input_per_million: 2.0,
            cached_input_per_million: 0.5,
            output_per_million: 8.0,
        };
    }

    PricingRate {
        input_per_million: 1.25,
        cached_input_per_million: 0.125,
        output_per_million: 10.0,
    }
}

#[cfg(test)]
mod tests;
