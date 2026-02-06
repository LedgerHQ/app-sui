from ledger_app_clients.exchange.cal_helper import CurrencyConfiguration
from application_client.sui_utils import SUI_CONF, SUI_PACKED_DERIVATION_PATH, SUI_USDC_CONF

SUI_CURRENCY_CONFIGURATION = CurrencyConfiguration(ticker="SUI", conf=SUI_CONF, packed_derivation_path=SUI_PACKED_DERIVATION_PATH)
SUI_USDC_CURRENCY_CONFIGURATION = CurrencyConfiguration(ticker="USDC", conf=SUI_USDC_CONF, packed_derivation_path=SUI_PACKED_DERIVATION_PATH)
