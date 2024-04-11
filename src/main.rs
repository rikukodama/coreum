use std::collections::HashMap;

fn main() {
    let original_balances = vec![
        balance("account1", vec![coin("denom1", 1000_000)]),
        balance("account2", vec![coin("denom1", 1000_000)]),
        balance("issuer_account_A", vec![coin("denom1", 1000_000)]),
    ];

    let definitions = vec![denom_definition("denom1", "issuer_account_A", 0.1, 0.0)];

    let multi_send_tx = MultiSend {
        inputs: vec![
            balance("account1", vec![coin("denom1", 60)]),
            balance("account2", vec![coin("denom1", 90)]),
            balance("issuer_account_A", vec![coin("denom1", 25)]),
        ],
        outputs: vec![
            balance("account_recipient_A", vec![coin("denom1", 50)]),
            balance("issuer_account_A", vec![coin("denom1", 100)]),
            balance("account_recipient_B", vec![coin("denom1", 25)]),
        ],
    };

    let result = calculate_balance_changes(original_balances, definitions, multi_send_tx);
    println!("{:#?}", result);
}

// A user can submit a `MultiSend` transaction (similar to bank.MultiSend in cosmos sdk) to transfer multiple
// coins (denoms) from multiple input addresses to multiple output addresses. A denom is the name or symbol
// for a coin type, e.g USDT and USDC can be considered different denoms; in cosmos ecosystem they are called
// denoms, in ethereum world they are called symbols.
// The sum of input coins and output coins must match for every transaction.
#[derive(Debug)]
struct MultiSend {
    // inputs contain the list of accounts that want to send coins from, and how many coins from each account we want to send.
    inputs: Vec<Balance>,
    // outputs contains the list of accounts that we want to deposit coins into, and how many coins to deposit into
    // each account
    outputs: Vec<Balance>,
}

#[derive(Debug, Clone)]
pub struct Coin {
    pub denom: String,
    pub amount: i128,
}

impl PartialEq for Coin {
    fn eq(&self, other: &Self) -> bool {
        self.denom == other.denom && self.amount == other.amount
    }
}

#[derive(Debug, Clone)]
struct Balance {
    address: String,
    coins: Vec<Coin>,
}

impl PartialEq for Balance {
    fn eq(&self, other: &Self) -> bool {
        if self.address == other.address {
            return self
                .coins
                .iter()
                .any(|coin| other.coins.iter().any(|other_coin| coin == other_coin));
        }
        false
    }
}

// A Denom has a definition (`CoinDefinition`) which contains different attributes related to the denom:
#[derive(Debug)]
struct DenomDefinition {
    // the unique identifier for the token (e.g `core`, `eth`, `usdt`, etc.)
    denom: String,
    // The address that created the token
    issuer: String,
    // burn_rate is a number between 0 and 1. If it is above zero, in every transfer,
    // some additional tokens will be burnt on top of the transferred value, from the senders address.
    // The tokens to be burnt are calculated by multiplying the TransferAmount by burn rate, and
    // rounding it up to an integer value. For example if an account sends 100 token and burn_rate is
    // 0.2, then 120 (100 + 100 * 0.2) will be deducted from sender account and 100 will be deposited to the recipient
    // account (i.e 20 tokens will be burnt)
    burn_rate: f64,
    // commission_rate is exactly same as the burn_rate, but the calculated value will be transferred to the
    // issuer's account address instead of being burnt.
    commission_rate: f64,
}

// Implement `calculate_balance_changes` with the following requirements.
// - Output of the function is the balance changes that must be applied to different accounts
//   (negative means deduction, positive means addition), or an error. the error indicates that the transaction must be rejected.
// - If sum of inputs and outputs in multi_send_tx does not match the tx must be rejected(i.e return error).
// - Apply burn_rate and commission_rate as described by their definition.
// - If the sender does not have enough balances (in the original_balances) to cover the input amount on top of burn_rate and
// commission_rate, the transaction must be rejected.
// - burn_rate and commission_rate does not apply to the issuer. So to calculate the correct values you must do this for every denom:
//      - sum all the inputs coming from accounts that are not an issuer (let's call it non_issuer_input_sum)
//      - sum all the outputs going to accounts that are not an issuer (let's call it non_issuer_output_sum)
//      - total burn amount is total_burn = min(non_issuer_input_sum, non_issuer_output_sum)
//      - total_burn is distributed between all input accounts as: account_share = roundup(total_burn * input_from_account / non_issuer_input_sum)
//      - total_burn_amount = sum (account_shares) // notice that in previous step we rounded up, so we need to recalculate the total again.
//      - commission_rate is exactly the same, but we send the calculate value to issuer, and not burn.
//      - Example:
//          burn_rate: 10%
//
//          inputs:
//          60, 90
//          25 <-- issuer
//
//          outputs:
//          50
//          100 <-- issuer
//          25
//          In this case burn amount is: min(non_issuer_inputs, non_issuer_outputs) = min(75+75, 50+25) = 75
//          Expected burn: 75 * 10% = 7.5
//          And now we divide it proportionally between all input sender: first_sender_share  = 7.5 * 60 / 150  = 3
//                                                                        second_sender_share = 7.5 * 90 / 150  = 4.5
// - In README.md we have provided more examples to help you better understand the requirements.
// - Write different unit tests to cover all the edge cases, we would like to see how you structure your tests.
//   There are examples in README.md, you can convert them into tests, but you should add more cases.
fn calculate_balance_changes(
    original_balances: Vec<Balance>,
    definitions: Vec<DenomDefinition>,
    multi_send_tx: MultiSend,
) -> Result<Vec<Balance>, String> {
    let mut result: HashMap<String, HashMap<String, i128>> = HashMap::new();
    let mut _original_balances = original_balances.clone();
    for balance in _original_balances {
        for coin in balance.coins {
            result
                .entry(balance.address.clone())
                .or_insert(HashMap::new())
                .insert(coin.denom.clone(), coin.amount);
        }
    }

    let mut definition_map: HashMap<String, DenomDefinition> = HashMap::new();

    for definition in definitions {
        definition_map.insert(definition.denom.clone(), definition);
    }

    let mut total_input: HashMap<String, i128> = HashMap::new();
    let mut total_output: HashMap<String, i128> = HashMap::new();
    let mut non_issuer_input: HashMap<String, i128> = HashMap::new();
    let mut non_issuer_output: HashMap<String, i128> = HashMap::new();

    for balance in &multi_send_tx.inputs {
        for coin in &balance.coins {
            if let Some(definition) = definition_map.get(&coin.denom) {
                let total_input = total_input.entry(coin.denom.clone()).or_insert(0);
                let non_issuer_input = non_issuer_input.entry(coin.denom.clone()).or_insert(0);
                *total_input += coin.amount;
                if definition.issuer != balance.address {
                    *non_issuer_input += coin.amount;
                }
            } else {
                return Err("Undefined definition".to_string());
            }
        }
    }

    for balance in &multi_send_tx.outputs {
        for coin in &balance.coins {
            if let Some(definition) = definition_map.get(&coin.denom) {
                let total_output = total_output.entry(coin.denom.clone()).or_insert(0);
                let non_issuer_output = non_issuer_output.entry(coin.denom.clone()).or_insert(0);
                *total_output += coin.amount;
                if definition.issuer != balance.address {
                    *non_issuer_output += coin.amount;
                }
            } else {
                return Err("Undefined definition".to_string());
            }
        }
    }

    for (denom, amount) in total_input.iter() {
        let output_amount = total_output.get(denom).unwrap_or(&0);
        if amount != output_amount {
            return Err("Input and output does not match".to_string());
        }
    }

    for balance in &multi_send_tx.inputs {
        for coin in &balance.coins {
            let definition = definition_map.get(&coin.denom).unwrap();

            let original_balance: &mut i128 = result
                .get_mut(&balance.address)
                .and_then(|denom_map| denom_map.get_mut(&coin.denom))
                .ok_or("Not enough balance".to_string())?;
            let non_issuer_input_val = non_issuer_input.get(&coin.denom).unwrap();
            let non_issuer_output_val = non_issuer_output.get(&coin.denom).unwrap();
            let mut burn_amount = non_issuer_input_val;
            if burn_amount > non_issuer_output_val {
                burn_amount = non_issuer_output_val;
            }
            let total_input = total_input.get(&coin.denom).unwrap();
            let mut burn = 0;
            let mut commission = 0;
            if definition.issuer != balance.address {
                burn = ((coin.amount * burn_amount / total_input) as f64 * definition.burn_rate)
                    .ceil() as i128;
                commission = ((coin.amount * burn_amount / total_input) as f64
                    * definition.commission_rate)
                    .ceil() as i128;
            }
            let new_amount = coin.amount + burn + commission;
            if *original_balance < new_amount {
                return Err("Not enough balance".to_string());
            }
            *original_balance -= new_amount;
            result
                .entry(definition.issuer.clone())
                .or_insert(HashMap::new())
                .entry(coin.denom.clone())
                .and_modify(|e| *e += commission)
                .or_insert(commission);
        }
    }

    for balance in &multi_send_tx.outputs {
        for coin in &balance.coins {
            let original_balance = result
                .entry(balance.address.clone())
                .or_insert(HashMap::new())
                .entry(coin.denom.clone())
                .or_insert(0);

            *original_balance += coin.amount;
        }
    }

    let mut final_balances: Vec<Balance> = vec![];

    for (address, coins_map) in result {
        let mut coins: Vec<Coin> = vec![];
        for (denom, amount) in coins_map {
            coins.push(Coin { denom, amount });
        }
        final_balances.push(Balance { address, coins });
    }

    let mut balance_changes: Vec<Balance> = Vec::new();

    for final_balance in final_balances {
        if let Some(original_balance) = original_balances
            .iter()
            .find(|&b| b.address == final_balance.address)
        {
            let mut change_coins = Vec::new();

            for final_coin in final_balance.coins {
                if let Some(original_coin) = original_balance
                    .coins
                    .iter()
                    .find(|&c| c.denom == final_coin.denom)
                {
                    change_coins.push(Coin {
                        denom: original_coin.denom.clone(),
                        amount: final_coin.amount - original_coin.amount,
                    });
                }
            }

            balance_changes.push(Balance {
                address: final_balance.address.clone(),
                coins: change_coins,
            });
        } else {
            if final_balance.coins.iter().all(|coin| coin.amount == 0) == false {
                balance_changes.push(final_balance);
            }
        }
    }
    Ok(balance_changes)
}

fn denom_definition(
    denom: &str,
    issuer: &str,
    burn_rate: f64,
    commission_rate: f64,
) -> DenomDefinition {
    DenomDefinition {
        denom: denom.to_string(),
        issuer: issuer.to_string(),
        burn_rate,
        commission_rate,
    }
}

fn coin(denom: &str, amount: i128) -> Coin {
    Coin {
        denom: denom.to_string(),
        amount,
    }
}

fn balance(address: &str, coins: Vec<Coin>) -> Balance {
    Balance {
        address: address.to_string(),
        coins,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_calculate_balance_changes(
        original_balances: Vec<Balance>,
        definitions: Vec<DenomDefinition>,
        multi_send_tx: MultiSend,
        expected_changes: Vec<Balance>,
    ) {
        let result = calculate_balance_changes(original_balances, definitions, multi_send_tx);
        assert!(result.is_ok());

        let changes = result.unwrap();
        assert_eq!(changes.len(), expected_changes.len());

        for change in changes {
            assert!(expected_changes.contains(&change));
        }
    }

    #[test]
    fn test_case_1() {
        let original_balances = vec![
            balance("account1", vec![coin("denom1", 1000_000)]),
            balance("account2", vec![coin("denom2", 1000_000)]),
        ];

        let definitions = vec![
            denom_definition("denom1", "issuer_account_A", 0.08, 0.12),
            denom_definition("denom2", "issuer_account_B", 1.0, 0.0),
        ];

        let multi_send_tx = MultiSend {
            inputs: vec![
                balance("account1", vec![coin("denom1", 1000)]),
                balance("account2", vec![coin("denom2", 1000)]),
            ],
            outputs: vec![balance(
                "account_recipient",
                vec![coin("denom1", 1000), coin("denom2", 1000)],
            )],
        };

        let expected_changes = vec![
            balance(
                "account_recipient",
                vec![coin("denom1", 1000), coin("denom2", 1000)],
            ),
            balance("issuer_account_A", vec![coin("denom1", 120)]),
            balance("account1", vec![coin("denom1", -1200)]),
            balance("account2", vec![coin("denom2", -2000)]),
        ];

        test_calculate_balance_changes(
            original_balances,
            definitions,
            multi_send_tx,
            expected_changes,
        );
    }

    #[test]
    fn test_case_2() {
        let original_balances = vec![
            balance("account1", vec![coin("denom1", 1000_000)]),
            balance("account2", vec![coin("denom1", 1000_000)]),
        ];

        let definitions = vec![denom_definition("denom1", "issuer_account_A", 0.08, 0.12)];

        let multi_send_tx = MultiSend {
            inputs: vec![
                balance("account1", vec![coin("denom1", 650)]),
                balance("account2", vec![coin("denom1", 350)]),
            ],
            outputs: vec![
                balance("account_recipient", vec![coin("denom1", 500)]),
                balance("issuer_account_A", vec![coin("denom1", 500)]),
            ],
        };

        let expected_changes = vec![
            balance("account_recipient", vec![coin("denom1", 500)]),
            balance("issuer_account_A", vec![coin("denom1", 560)]),
            balance("account1", vec![coin("denom1", -715)]),
            balance("account2", vec![coin("denom1", -385)]),
        ];

        test_calculate_balance_changes(
            original_balances,
            definitions,
            multi_send_tx,
            expected_changes,
        );
    }

    #[test]
    fn test_case_3() {
        let original_balances = vec![
            balance(
                "account1",
                vec![coin("denom1", 1000_000), coin("denom2", 1000_000)],
            ),
            balance(
                "account2",
                vec![coin("denom1", 1000_000), coin("denom2", 1000_000)],
            ),
        ];

        let definitions = vec![
            denom_definition("denom1", "issuer_account_A", 0.08, 0.12),
            denom_definition("denom2", "issuer_account_A", 1.0, 0.0),
        ];

        let multi_send_tx = MultiSend {
            inputs: vec![
                balance("account1", vec![coin("denom1", 650), coin("denom2", 300)]),
                balance("account2", vec![coin("denom1", 350), coin("denom2", 500)]),
            ],
            outputs: vec![
                balance(
                    "account_recipient",
                    vec![coin("denom1", 500), coin("denom2", 500)],
                ),
                balance(
                    "issuer_account_A",
                    vec![coin("denom1", 500), coin("denom2", 300)],
                ),
            ],
        };

        let expected_changes = vec![
            balance(
                "account_recipient",
                vec![coin("denom1", 500), coin("denom2", 500)],
            ),
            balance(
                "issuer_account_A",
                vec![coin("denom1", 560), coin("denom2", 300)],
            ),
            balance("account1", vec![coin("denom1", -715), coin("denom1", -487)]),
            balance("account2", vec![coin("denom2", -385), coin("denom2", -812)]),
        ];

        test_calculate_balance_changes(
            original_balances,
            definitions,
            multi_send_tx,
            expected_changes,
        );
    }

    #[test]
    fn test_case_4() {
        let original_balances = vec![
            balance("account1", vec![coin("denom1", 1000)]),
            balance("account2", vec![coin("denom1", 1000)]),
        ];

        let definitions = vec![denom_definition("denom1", "issuer_account_A", 0.01, 0.01)];

        let multi_send_tx = MultiSend {
            inputs: vec![
                balance("account1", vec![coin("denom1", 1)]),
                balance("account2", vec![coin("denom1", 1)]),
            ],
            outputs: vec![balance("account_recipient", vec![coin("denom1", 2)])],
        };

        let expected_changes = vec![
            balance("account_recipient", vec![coin("denom1", 2)]),
            balance("issuer_account_A", vec![coin("denom1", 2)]),
            balance("account1", vec![coin("denom1", -3)]),
            balance("account2", vec![coin("denom1", -3)]),
        ];

        test_calculate_balance_changes(
            original_balances,
            definitions,
            multi_send_tx,
            expected_changes,
        );
    }

    #[test]
    fn test_case_5() {
        let original_balances = vec![
            balance("account1", vec![coin("denom1", 1000_000)]),
            balance("account2", vec![coin("denom1", 1000_000)]),
            balance("issuer_account_A", vec![coin("denom1", 1000_000)]),
        ];

        let definitions = vec![denom_definition("denom1", "issuer_account_A", 0.1, 0.0)];

        let multi_send_tx = MultiSend {
            inputs: vec![
                balance("account1", vec![coin("denom1", 60)]),
                balance("account2", vec![coin("denom1", 90)]),
                balance("issuer_account_A", vec![coin("denom1", 25)]),
            ],
            outputs: vec![
                balance("account_recipient_A", vec![coin("denom1", 50)]),
                balance("issuer_account_A", vec![coin("denom1", 100)]),
                balance("account_recipient_B", vec![coin("denom1", 25)]),
            ],
        };

        let expected_changes = vec![
            balance("account_recipient_A", vec![coin("denom1", 50)]),
            balance("account_recipient_B", vec![coin("denom1", 25)]),
            balance("issuer_account_A", vec![coin("denom1", 75)]),
            balance("account1", vec![coin("denom1", -63)]),
            balance("account2", vec![coin("denom1", -94)]),
        ];

        test_calculate_balance_changes(
            original_balances,
            definitions,
            multi_send_tx,
            expected_changes,
        );
    }
    
    /// Error Cases
    #[test]
    fn test_case_6() {
        let original_balances = vec![
            balance("account1", vec![coin("denom1", 1000)]),
            balance("account2", vec![coin("denom2", 1000)]),
        ];

        let definitions = vec![
            denom_definition("denom1", "issuer_account_A", 0.08, 0.12),
            denom_definition("denom2", "issuer_account_B", 1.0, 0.0),
        ];

        let multi_send_tx = MultiSend {
            inputs: vec![
                balance("account1", vec![coin("denom1", 1000)]),
                balance("account2", vec![coin("denom2", 1000)]),
            ],
            outputs: vec![balance(
                "account_recipient",
                vec![coin("denom1", 1000), coin("denom2", 1000)],
            )],
        };

        let result = calculate_balance_changes(
            original_balances,
            definitions,
            multi_send_tx,
        );
        
        assert!(result.is_err())
    }
    
    #[test]
    fn test_case_7() {
        let original_balances = vec![
            balance("account1", vec![coin("denom1", 2000)]),
            balance("account2", vec![coin("denom2", 2000)]),
        ];

        let definitions = vec![
            denom_definition("denom1", "issuer_account_A", 0.08, 0.12),
            denom_definition("denom2", "issuer_account_B", 1.0, 0.0),
        ];

        let multi_send_tx = MultiSend {
            inputs: vec![
                balance("account1", vec![coin("denom1", 1000)]),
                balance("account2", vec![coin("denom2", 1000)]),
            ],
            outputs: vec![balance(
                "account_recipient",
                vec![coin("denom1", 1500), coin("denom2", 1000)],
            )],
        };

        let result = calculate_balance_changes(
            original_balances,
            definitions,
            multi_send_tx,
        );

        assert!(result.is_err());
    }
    // Add more tests here to cover additional cases and corner cases
}
