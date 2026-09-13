use once_cell::sync::Lazy;
use regex::Regex;

use crate::models::TransactionType;

type Rule = (Regex, &'static str, f64);

fn rule(pattern: &str, category: &'static str, confidence: f64) -> Rule {
    (
        Regex::new(&format!("(?i){pattern}")).expect("valid category regex"),
        category,
        confidence,
    )
}

static DEBIT_RULES: Lazy<Vec<Rule>> = Lazy::new(|| {
    vec![
        rule(r"APPLE\.COM/BILL", "subscription", 0.95),
        rule(r"Google One", "subscription", 0.95),
        rule(r"AMAZON PRIME", "subscription", 0.95),
        rule(r"COBRO DE MEMBRESIA", "subscription", 0.90),
        rule(r"NETFLIX", "subscription", 0.95),
        rule(r"SPOTIFY", "subscription", 0.95),
        rule(r"DISNEY\+", "subscription", 0.95),
        rule(r"UBER RIDES", "transport", 0.95),
        rule(r"DL Uber", "transport", 0.95),
        rule(r"SISTEMA DE TRANSPORTE", "transport", 0.95),
        rule(r"CABIFY", "transport", 0.95),
        rule(r"DIDI", "transport", 0.95),
        rule(r"SUPER\s*99", "groceries", 0.95),
        rule(r"SUPER\s*XTRA", "groceries", 0.95),
        rule(r"EL MACHETAZO", "groceries", 0.95),
        rule(r"SUPER\s*7", "groceries", 0.90),
        rule(r"MINI SUPER", "groceries", 0.85),
        rule(r"REY\s", "groceries", 0.85),
        rule(r"RIBA SMITH", "groceries", 0.90),
        rule(r"RESTAURANTE", "food_dining", 0.90),
        rule(r"BEERMARKT", "food_dining", 0.90),
        rule(r"PANDA HOUSE", "food_dining", 0.90),
        rule(r"MCDON", "food_dining", 0.95),
        rule(r"STARBUCKS", "food_dining", 0.95),
        rule(r"KFC", "food_dining", 0.95),
        rule(r"WENDYS", "food_dining", 0.95),
        rule(r"DOMINOS", "food_dining", 0.95),
        rule(r"NATURGY", "utilities", 0.95),
        rule(r"EDEMET", "utilities", 0.95),
        rule(r"EDECHI", "utilities", 0.95),
        rule(r"CABLE AND WIRELESS", "utilities", 0.95),
        rule(r"IDAAN", "utilities", 0.95),
        rule(r"RECARGA TELEFONIA", "utilities", 0.90),
        rule(r"FARMACIA", "health", 0.90),
        rule(r"ARROCHA", "health", 0.85),
        rule(r"HOSPITAL", "health", 0.90),
        rule(r"CLINICA", "health", 0.90),
        rule(r"AMAZON MKTPL", "online_shopping", 0.95),
        rule(r"AMAZON RETA", "online_shopping", 0.95),
        rule(r"MERCADOLIBRE", "online_shopping", 0.95),
        rule(r"EBAY", "online_shopping", 0.95),
        rule(r"FACTORY FASHION", "shopping", 0.85),
        rule(r"ALMACEN", "shopping", 0.70),
        rule(r"LA OCA", "shopping", 0.80),
        rule(r"ITBMS", "bank_fee", 0.90),
        rule(r"IMPUESTO.*FRAUDE", "bank_fee", 0.95),
        rule(r"SEGURO DE FRAUDE", "bank_fee", 0.95),
        rule(r"COMISION", "bank_fee", 0.90),
        rule(r"ATM-", "atm_withdrawal", 0.95),
        rule(r"YAPPY\s+A\s+", "p2p_payment", 0.95),
        rule(r"PAGO YAPPY", "p2p_payment", 0.95),
        rule(r"TRANSFERENCIA\s+A\s+.*ENTRE CUENTAS", "transfer_out", 0.99),
    ]
});

static CREDIT_RULES: Lazy<Vec<Rule>> = Lazy::new(|| {
    vec![
        rule(r"TRANSFERENCIA\s+DE\s+.*ENTRE CUENTAS", "transfer_in", 0.99),
        rule(r"YAPPY\s+OB\s+DE", "gift", 0.70),
        rule(r"INTERES\s+CUENTA", "interest", 0.99),
        rule(r"NOMINA", "income", 0.95),
        rule(r"SALARIO", "income", 0.95),
        rule(r"PAYROLL", "income", 0.95),
    ]
});

static CARD_MASK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"-(\d{4}-\d{2}XX-XXXX-\d{4})$").expect("valid card-mask regex"));

static RECURRING_RULES: Lazy<Vec<Regex>> = Lazy::new(|| {
    [
        r"APPLE\.COM/BILL",
        r"Google One",
        r"AMAZON PRIME",
        r"NETFLIX",
        r"SPOTIFY",
        r"COBRO DE MEMBRESIA",
        r"SEGURO DE FRAUDE MENSU",
        r"NATURGY",
        r"CABLE AND WIRELESS",
        r"NOMINA",
        r"SALARIO",
        r"PAYROLL",
    ]
    .into_iter()
    .map(|pattern| Regex::new(&format!("(?i){pattern}")).expect("valid recurring regex"))
    .collect()
});

pub fn categorize(memo: &str, amount_minor: i64) -> (TransactionType, String, f64) {
    let (transaction_type, rules, fallback) = if amount_minor >= 0 {
        (TransactionType::Credit, &*CREDIT_RULES, "other_credit")
    } else {
        (TransactionType::Debit, &*DEBIT_RULES, "other_expense")
    };

    for (pattern, category, confidence) in rules {
        if pattern.is_match(memo) {
            return (transaction_type, (*category).to_string(), *confidence);
        }
    }

    (transaction_type, fallback.to_string(), 0.30)
}

pub fn clean_vendor(memo: &str) -> (String, Option<String>, String) {
    let mut clean = memo.trim().to_string();
    let card_mask = CARD_MASK
        .captures(&clean)
        .and_then(|captures| captures.get(1).map(|value| value.as_str().to_string()));

    if let Some(found) = CARD_MASK.find(&clean) {
        clean.truncate(found.start());
    }
    clean = clean.trim().trim_end_matches('-').trim().to_string();

    let mut normalized = clean
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    let special = [
        ("uber rides", "Uber"),
        ("dl uber rides", "Uber"),
        ("apple.com/bill", "Apple"),
        ("google one", "Google One"),
        ("super xtra transismica", "Super Xtra"),
        ("super 99 tumba muerto", "Super 99"),
    ];
    let lower = normalized.to_lowercase();
    if let Some((_, replacement)) = special.iter().find(|(pattern, _)| lower.contains(pattern)) {
        normalized = (*replacement).to_string();
    }

    (clean, card_mask, normalized)
}

pub fn detect_recurring(memo: &str) -> Option<bool> {
    RECURRING_RULES
        .iter()
        .any(|pattern| pattern.is_match(memo))
        .then_some(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorizes_and_cleans_known_values() {
        let (_, category, confidence) = categorize("APPLE.COM/BILL-4187-94XX-XXXX-6463", -999);
        assert_eq!(category, "subscription");
        assert_eq!(confidence, 0.95);
        let (clean, mask, normalized) = clean_vendor("APPLE.COM/BILL-4187-94XX-XXXX-6463");
        assert_eq!(clean, "APPLE.COM/BILL");
        assert_eq!(mask.as_deref(), Some("4187-94XX-XXXX-6463"));
        assert_eq!(normalized, "Apple");
        assert_eq!(detect_recurring("PAYROLL ACME"), Some(true));
    }
}
