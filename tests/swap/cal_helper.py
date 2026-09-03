from ledger_app_clients.exchange.cal_helper import CurrencyConfiguration
from application_client.sui_utils import (
    SUI_CONF,
    SUI_OVERSIZED_DECIMALS_CONF,
    SUI_PACKED_DERIVATION_PATH,
    SUI_UNKNOWN_CONF,
    SUI_USDC_CONF,
)

SUI_CURRENCY_CONFIGURATION = CurrencyConfiguration(ticker="SUI", conf=SUI_CONF, packed_derivation_path=SUI_PACKED_DERIVATION_PATH)
SUI_USDC_CURRENCY_CONFIGURATION = CurrencyConfiguration(ticker="USDC", conf=SUI_USDC_CONF, packed_derivation_path=SUI_PACKED_DERIVATION_PATH)
# Swap whose ticker is not in the app's KNOWN_COINS table.
SUI_UNKNOWN_CURRENCY_CONFIGURATION = CurrencyConfiguration(ticker="TADA", conf=SUI_UNKNOWN_CONF, packed_derivation_path=SUI_PACKED_DERIVATION_PATH)
# Coin config whose decimals exceed what the app can render.
SUI_OVERSIZED_DECIMALS_CURRENCY_CONFIGURATION = CurrencyConfiguration(ticker="USDC", conf=SUI_OVERSIZED_DECIMALS_CONF, packed_derivation_path=SUI_PACKED_DERIVATION_PATH)
