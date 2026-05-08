For batch flush trigger, when ANY of these conditions are met:
1. fills >= MAX_BATCH_SIZE (eg 50)                               - size cap
2. elapsed >= MAX_BATCH_AGE_MS (eg 500ms)                        - latency cap
3. gas_price <= GAS_PRICE_THRESHOLD                              - opportunistic flush ( dynamic gas price trigger)
4. fills >= MIN_BATCH_SIZE && gas_price is below rolling average - smart: only flush small batches when gas is cheap.