# Docker Commands for app-sui Development

## Build (apex_p)

```bash
docker exec --user 1000:1000 -it app-sui-container bash -c 'cd ./rust-app/ && cargo ledger build apex_p -- -Zunstable-options --out-dir build/apex_p/bin && mv build/apex_p/bin/sui build/apex_p/bin/app.elf && mv build/apex_p/bin/sui.apdu build/apex_p/bin/app.apdu'
```

## Test (FundsWithdrawal, apex_p)

```bash
docker exec --user 1000:1000 -it app-sui-container bash -c "source /opt/venv/bin/activate && pytest ./tests/standalone/test_sign_sui_funds_withdrawal.py --tb=short -v -s --device apex_p -W ignore::DeprecationWarning -W ignore::UserWarning"
```

**Note:** If running from a non-TTY context (e.g. CI), omit `-it`:

```bash
docker exec --user 1000:1000 app-sui-container bash -c "source /opt/venv/bin/activate && pytest ./tests/standalone/test_sign_sui_funds_withdrawal.py --tb=short -v -s --device apex_p -W ignore::DeprecationWarning -W ignore::UserWarning"
```
