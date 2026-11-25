# App against mock

360 app captures are paired with a mock capture of the same theme and state id. SSIM is 1 for identical images; lower is a bigger difference, and the list runs worst first.

| Theme | State | SSIM |
| --- | --- | --- |
| hc | home.new-tab | 0.4411 |
| hc | home.nudge-open | 0.4478 |
| hc | sales-invoice.save-confirm | 0.4772 |
| hc | home.default | 0.4778 |
| hc | home.gst-unsupported | 0.4804 |
| hc | bank.default | 0.4966 |
| hc | profit-and-loss.provisional | 0.5009 |
| hc | audit.default | 0.5030 |
| hc | profit-and-loss.print-failed | 0.5090 |
| hc | day-book.month | 0.5094 |
| hc | balance-sheet.provisional | 0.5160 |
| hc | masters.loading | 0.5166 |
| hc | palette.entry | 0.5197 |
| hc | palette.ask | 0.5225 |
| hc | palette.voucher | 0.5312 |
| dark | home.new-tab | 0.5329 |
| light | home.new-tab | 0.5340 |
| light | home.default | 0.5371 |
| hc | palette.no-results | 0.5374 |
| dark | home.default | 0.5421 |
| hc | profit-and-loss.detailed | 0.5429 |
| hc | sales-invoice.validation-error | 0.5437 |
| hc | ai-settings.cloud | 0.5451 |
| light | home.nudge-open | 0.5452 |
| hc | bank.offline-ai | 0.5477 |
| hc | review.default | 0.5479 |
| hc | stock-summary.provisional | 0.5496 |
| hc | ai-settings.local | 0.5533 |
| dark | home.nudge-open | 0.5637 |
| hc | trial-balance.default | 0.5691 |
| dark | profit-and-loss.print-preview | 0.5716 |
| hc | import.imported | 0.5720 |
| hc | masters.ledgers | 0.5753 |
| light | home.gst-unsupported | 0.5756 |
| hc | import.no-company | 0.5782 |
| light | audit.default | 0.5783 |
| dark | bank.default | 0.5784 |
| hc | home.ai-off | 0.5785 |
| dark | home.gst-unsupported | 0.5790 |
| hc | palette.search | 0.5813 |
| hc | profit-and-loss.print-preview | 0.5825 |
| hc | profit-and-loss.print-busy | 0.5839 |
| light | palette.entry | 0.5862 |
| light | palette.ask | 0.5890 |
| hc | profit-and-loss.print-saved | 0.5904 |
| hc | sales-invoice.item-list | 0.5916 |
| hc | import.port-error | 0.5924 |
| hc | import.validation-error | 0.5934 |
| dark | audit.default | 0.5945 |
| hc | import.failed | 0.5947 |
| hc | import.refused | 0.5951 |
| hc | palette.default | 0.5970 |
| hc | sales-invoice.period-locked | 0.5974 |
| light | palette.voucher | 0.5980 |
| light | palette.no-results | 0.5990 |
| hc | keys.standard | 0.6004 |
| hc | purchase.validation-error | 0.6011 |
| hc | balance-sheet.detailed | 0.6021 |
| light | masters.ledgers | 0.6024 |
| light | review.default | 0.6026 |
| hc | gst.gstr1-correction | 0.6033 |
| hc | gst.gstr1 | 0.6034 |
| hc | purchase-bill.offline-ai | 0.6035 |
| dark | review.default | 0.6039 |
| light | sales-invoice.save-confirm | 0.6043 |
| light | bank.default | 0.6059 |
| dark | bank.offline-ai | 0.6067 |
| hc | masters.items | 0.6068 |
| hc | keys.classic | 0.6069 |
| hc | balance-sheet.default | 0.6087 |
| light | balance-sheet.provisional | 0.6118 |
| light | ai-settings.cloud | 0.6137 |
| hc | masters.no-match | 0.6139 |
| light | day-book.month | 0.6157 |
| dark | palette.entry | 0.6165 |
| hc | sales-invoice.gst-inter-state | 0.6165 |
| light | trial-balance.default | 0.6168 |
| dark | sales-invoice.save-confirm | 0.6184 |
| light | sales-invoice.period-locked | 0.6188 |
| dark | palette.ask | 0.6191 |
| hc | sales-invoice.default | 0.6222 |
| dark | palette.no-results | 0.6241 |
| dark | ai-settings.cloud | 0.6247 |
| light | profit-and-loss.detailed | 0.6250 |
| dark | day-book.month | 0.6269 |
| light | bank.offline-ai | 0.6272 |
| dark | palette.voucher | 0.6284 |
| light | sales-invoice.default | 0.6296 |
| light | palette.search | 0.6303 |
| light | sales-invoice.item-list | 0.6306 |
| hc | import.running | 0.6307 |
| dark | masters.ledgers | 0.6330 |
| hc | palette.commands | 0.6333 |
| light | sales-invoice.gst-inter-state | 0.6339 |
| hc | ai-settings.off | 0.6367 |
| light | sales-invoice.validation-error | 0.6367 |
| dark | sales-invoice.period-locked | 0.6384 |
| hc | profit-and-loss.default | 0.6387 |
| dark | sales-invoice.item-list | 0.6394 |
| light | profit-and-loss.provisional | 0.6412 |
| dark | sales-invoice.validation-error | 0.6421 |
| dark | profit-and-loss.provisional | 0.6446 |
| dark | balance-sheet.provisional | 0.6449 |
| hc | purchase.default | 0.6458 |
| hc | sales-invoice.b2c | 0.6468 |
| hc | import.default | 0.6475 |
| light | profit-and-loss.print-preview | 0.6486 |
| dark | sales-invoice.default | 0.6502 |
| light | profit-and-loss.print-busy | 0.6505 |
| dark | trial-balance.default | 0.6510 |
| dark | palette.search | 0.6519 |
| dark | purchase-bill.offline-ai | 0.6543 |
| light | profit-and-loss.print-failed | 0.6543 |
| dark | profit-and-loss.detailed | 0.6545 |
| light | keys.standard | 0.6546 |
| light | balance-sheet.detailed | 0.6556 |
| dark | keys.standard | 0.6557 |
| dark | sales-invoice.gst-inter-state | 0.6557 |
| light | palette.default | 0.6565 |
| dark | profit-and-loss.print-failed | 0.6569 |
| hc | payment.overdraft | 0.6571 |
| hc | credit-note.validation-error | 0.6581 |
| hc | bank.brs | 0.6585 |
| dark | stock-summary.provisional | 0.6590 |
| light | stock-summary.provisional | 0.6591 |
| hc | gst.gstr3b | 0.6594 |
| hc | sales-invoice.saved | 0.6601 |
| light | ai-settings.local | 0.6612 |
| light | sales-invoice.b2c | 0.6613 |
| hc | debit-note.validation-error | 0.6618 |
| light | balance-sheet.default | 0.6633 |
| dark | profit-and-loss.print-busy | 0.6646 |
| hc | balance-sheet.voucher | 0.6647 |
| light | keys.classic | 0.6648 |
| dark | keys.classic | 0.6651 |
| hc | balance-sheet.ledger | 0.6653 |
| dark | ai-settings.local | 0.6668 |
| dark | home.ai-off | 0.6673 |
| light | home.ai-off | 0.6705 |
| dark | import.no-company | 0.6737 |
| light | gst.gstr1 | 0.6737 |
| hc | outstanding.receivables | 0.6741 |
| dark | palette.default | 0.6744 |
| light | palette.commands | 0.6771 |
| dark | gst.gstr1 | 0.6781 |
| light | import.no-company | 0.6803 |
| dark | import.refused | 0.6807 |
| light | sales-invoice.saved | 0.6811 |
| dark | sales-invoice.b2c | 0.6815 |
| light | import.refused | 0.6820 |
| dark | import.failed | 0.6823 |
| dark | import.imported | 0.6827 |
| hc | journal.validation-error | 0.6831 |
| light | import.failed | 0.6833 |
| light | gst.gstr1-correction | 0.6846 |
| dark | import.port-error | 0.6857 |
| light | ai-settings.off | 0.6872 |
| light | import.imported | 0.6872 |
| dark | import.validation-error | 0.6872 |
| dark | gst.gstr1-correction | 0.6893 |
| hc | receipt.default | 0.6896 |
| light | outstanding.receivables | 0.6902 |
| dark | ai-settings.off | 0.6917 |
| light | import.port-error | 0.6918 |
| dark | masters.no-match | 0.6924 |
| hc | payment.bill-wise | 0.6925 |
| light | import.validation-error | 0.6931 |
| dark | balance-sheet.detailed | 0.6955 |
| dark | sales-invoice.saved | 0.6963 |
| dark | import.running | 0.6974 |
| dark | import.default | 0.6977 |
| dark | palette.commands | 0.6980 |
| light | masters.no-match | 0.6987 |
| light | profit-and-loss.print-saved | 0.6989 |
| light | profit-and-loss.default | 0.6992 |
| dark | balance-sheet.default | 0.7009 |
| light | gst.gstr3b | 0.7014 |
| light | import.running | 0.7014 |
| dark | profit-and-loss.print-saved | 0.7022 |
| dark | outstanding.receivables | 0.7027 |
| hc | credit-note.default | 0.7034 |
| light | import.default | 0.7051 |
| light | purchase.default | 0.7062 |
| hc | balance-sheet.group | 0.7072 |
| light | purchase.validation-error | 0.7079 |
| dark | gst.gstr3b | 0.7083 |
| dark | purchase.validation-error | 0.7085 |
| hc | gst.recon | 0.7086 |
| hc | setup.features | 0.7087 |
| hc | debit-note.default | 0.7095 |
| hc | contra.default | 0.7098 |
| hc | journal.default | 0.7104 |
| hc | payment.default | 0.7107 |
| hc | stock-summary.default | 0.7111 |
| dark | profit-and-loss.default | 0.7111 |
| light | payment.overdraft | 0.7138 |
| dark | purchase.default | 0.7148 |
| dark | masters.items | 0.7172 |
| light | bank.brs | 0.7175 |
| light | masters.loading | 0.7179 |
| dark | payment.overdraft | 0.7186 |
| dark | bank.brs | 0.7190 |
| light | masters.items | 0.7195 |
| hc | setup.new-tab | 0.7204 |
| hc | home.loading | 0.7220 |
| light | balance-sheet.voucher | 0.7230 |
| dark | masters.loading | 0.7258 |
| hc | audit.integrity | 0.7265 |
| light | receipt.default | 0.7267 |
| light | stock-summary.default | 0.7276 |
| light | balance-sheet.group | 0.7285 |
| light | payment.bill-wise | 0.7289 |
| light | credit-note.validation-error | 0.7320 |
| light | gst.recon | 0.7321 |
| dark | balance-sheet.voucher | 0.7326 |
| hc | contra.validation-error | 0.7329 |
| light | debit-note.validation-error | 0.7341 |
| hc | outstanding.payables | 0.7357 |
| dark | receipt.default | 0.7358 |
| dark | balance-sheet.ledger | 0.7363 |
| dark | credit-note.validation-error | 0.7366 |
| hc | day-book.default | 0.7368 |
| light | balance-sheet.ledger | 0.7371 |
| hc | masters.ledger-detail | 0.7373 |
| dark | balance-sheet.group | 0.7375 |
| light | outstanding.payables | 0.7378 |
| dark | gst.recon | 0.7383 |
| dark | payment.bill-wise | 0.7384 |
| dark | debit-note.validation-error | 0.7389 |
| dark | stock-summary.default | 0.7402 |
| light | purchase-bill.offline-ai | 0.7449 |
| dark | outstanding.payables | 0.7461 |
| hc | bank.import-error | 0.7478 |
| hc | profit-and-loss.loading | 0.7488 |
| light | payment.default | 0.7498 |
| hc | masters.create-ledger | 0.7521 |
| dark | payment.default | 0.7562 |
| light | day-book.default | 0.7562 |
| hc | home.empty | 0.7590 |
| light | setup.features | 0.7592 |
| hc | balance-sheet.loading | 0.7601 |
| light | credit-note.default | 0.7618 |
| hc | masters.create-item | 0.7620 |
| hc | masters.gstin-error | 0.7626 |
| hc | ask.offline-ai | 0.7641 |
| light | journal.validation-error | 0.7642 |
| light | contra.default | 0.7646 |
| light | setup.new-tab | 0.7648 |
| dark | day-book.default | 0.7656 |
| hc | stock-summary.loading | 0.7665 |
| light | debit-note.default | 0.7670 |
| dark | credit-note.default | 0.7671 |
| hc | audit.loading | 0.7672 |
| hc | outstanding.loading | 0.7674 |
| dark | journal.validation-error | 0.7678 |
| hc | trial-balance.loading | 0.7680 |
| dark | setup.features | 0.7682 |
| dark | setup.new-tab | 0.7700 |
| dark | contra.default | 0.7703 |
| dark | debit-note.default | 0.7725 |
| hc | setup.company | 0.7763 |
| dark | masters.ledger-detail | 0.7769 |
| light | journal.default | 0.7782 |
| light | masters.ledger-detail | 0.7802 |
| light | contra.validation-error | 0.7815 |
| dark | journal.default | 0.7824 |
| hc | bank.empty | 0.7837 |
| dark | contra.validation-error | 0.7882 |
| hc | gst.recon-locked | 0.7901 |
| hc | masters.items-empty | 0.7925 |
| hc | masters.empty | 0.7935 |
| dark | setup.company | 0.7954 |
| hc | audit.unsupported | 0.7961 |
| light | masters.create-ledger | 0.7965 |
| light | setup.company | 0.7970 |
| dark | masters.create-ledger | 0.7974 |
| light | masters.gstin-error | 0.7984 |
| light | audit.integrity | 0.7985 |
| dark | bank.import-error | 0.7989 |
| dark | masters.gstin-error | 0.7989 |
| hc | setup.gstin-error | 0.7989 |
| dark | home.empty | 0.8001 |
| dark | audit.integrity | 0.8017 |
| light | home.loading | 0.8022 |
| light | audit.loading | 0.8025 |
| hc | gst.recon-empty | 0.8038 |
| light | profit-and-loss.loading | 0.8044 |
| light | home.empty | 0.8046 |
| hc | setup.backup | 0.8052 |
| hc | setup.ai | 0.8052 |
| light | bank.import-error | 0.8052 |
| hc | day-book.empty | 0.8067 |
| dark | masters.create-item | 0.8070 |
| light | masters.create-item | 0.8075 |
| light | ask.offline-ai | 0.8077 |
| dark | bank.empty | 0.8108 |
| light | bank.empty | 0.8112 |
| hc | bank-chooser.default | 0.8112 |
| hc | review.empty | 0.8123 |
| dark | gst.recon-locked | 0.8128 |
| dark | ask.offline-ai | 0.8133 |
| light | gst.recon-locked | 0.8135 |
| dark | setup.gstin-error | 0.8144 |
| hc | setup.welcome | 0.8147 |
| light | setup.gstin-error | 0.8154 |
| light | balance-sheet.loading | 0.8160 |
| hc | gst.recon-error | 0.8173 |
| light | stock-summary.loading | 0.8203 |
| hc | setup.keyboard | 0.8206 |
| dark | profit-and-loss.loading | 0.8206 |
| light | outstanding.loading | 0.8213 |
| light | trial-balance.loading | 0.8220 |
| light | audit.unsupported | 0.8249 |
| dark | audit.unsupported | 0.8251 |
| dark | home.loading | 0.8256 |
| dark | masters.items-empty | 0.8272 |
| dark | masters.empty | 0.8286 |
| light | gst.recon-empty | 0.8296 |
| dark | gst.recon-empty | 0.8302 |
| light | masters.items-empty | 0.8309 |
| dark | balance-sheet.loading | 0.8311 |
| dark | review.empty | 0.8316 |
| light | masters.empty | 0.8319 |
| light | review.empty | 0.8335 |
| dark | audit.loading | 0.8338 |
| light | setup.backup | 0.8350 |
| light | day-book.empty | 0.8360 |
| dark | stock-summary.loading | 0.8363 |
| dark | day-book.empty | 0.8367 |
| dark | outstanding.loading | 0.8372 |
| dark | setup.backup | 0.8374 |
| dark | trial-balance.loading | 0.8379 |
| light | setup.welcome | 0.8417 |
| hc | gst.gstr3b-unsupported | 0.8426 |
| dark | gst.recon-error | 0.8429 |
| light | setup.keyboard | 0.8440 |
| dark | setup.keyboard | 0.8444 |
| hc | start.recent | 0.8450 |
| hc | setup.rail | 0.8450 |
| light | bank-chooser.default | 0.8459 |
| dark | bank-chooser.default | 0.8470 |
| dark | setup.welcome | 0.8473 |
| light | gst.recon-error | 0.8477 |
| light | setup.ai | 0.8483 |
| hc | gst.gstr1-not-loaded | 0.8492 |
| hc | gst.gstr3b-not-loaded | 0.8511 |
| dark | setup.ai | 0.8513 |
| dark | gst.gstr3b-unsupported | 0.8559 |
| light | gst.gstr3b-unsupported | 0.8593 |
| light | gst.gstr1-not-loaded | 0.8617 |
| light | gst.gstr3b-not-loaded | 0.8622 |
| dark | gst.gstr1-not-loaded | 0.8631 |
| dark | gst.gstr3b-not-loaded | 0.8637 |
| light | setup.rail | 0.8661 |
| dark | start.recent | 0.8664 |
| hc | setup.done | 0.8667 |
| light | start.recent | 0.8669 |
| dark | setup.rail | 0.8719 |
| light | setup.done | 0.8926 |
| dark | setup.done | 0.8950 |

## hc / home.new-tab

SSIM 0.4411

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/home.new-tab.webp" width="480" alt="mock hc home.new-tab"></td>
<td><img src="app-captures/hc/home.new-tab.webp" width="480" alt="app hc home.new-tab"></td>
</tr>
</table>

## hc / home.nudge-open

SSIM 0.4478

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/home.nudge-open.webp" width="480" alt="mock hc home.nudge-open"></td>
<td><img src="app-captures/hc/home.nudge-open.webp" width="480" alt="app hc home.nudge-open"></td>
</tr>
</table>

## hc / sales-invoice.save-confirm

SSIM 0.4772

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.save-confirm.webp" width="480" alt="mock hc sales-invoice.save-confirm"></td>
<td><img src="app-captures/hc/sales-invoice.save-confirm.webp" width="480" alt="app hc sales-invoice.save-confirm"></td>
</tr>
</table>

## hc / home.default

SSIM 0.4778

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/home.default.webp" width="480" alt="mock hc home.default"></td>
<td><img src="app-captures/hc/home.default.webp" width="480" alt="app hc home.default"></td>
</tr>
</table>

## hc / home.gst-unsupported

SSIM 0.4804

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/home.gst-unsupported.webp" width="480" alt="mock hc home.gst-unsupported"></td>
<td><img src="app-captures/hc/home.gst-unsupported.webp" width="480" alt="app hc home.gst-unsupported"></td>
</tr>
</table>

## hc / bank.default

SSIM 0.4966

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/bank.default.webp" width="480" alt="mock hc bank.default"></td>
<td><img src="app-captures/hc/bank.default.webp" width="480" alt="app hc bank.default"></td>
</tr>
</table>

## hc / profit-and-loss.provisional

SSIM 0.5009

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.provisional.webp" width="480" alt="mock hc profit-and-loss.provisional"></td>
<td><img src="app-captures/hc/profit-and-loss.provisional.webp" width="480" alt="app hc profit-and-loss.provisional"></td>
</tr>
</table>

## hc / audit.default

SSIM 0.5030

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/audit.default.webp" width="480" alt="mock hc audit.default"></td>
<td><img src="app-captures/hc/audit.default.webp" width="480" alt="app hc audit.default"></td>
</tr>
</table>

## hc / profit-and-loss.print-failed

SSIM 0.5090

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.print-failed.webp" width="480" alt="mock hc profit-and-loss.print-failed"></td>
<td><img src="app-captures/hc/profit-and-loss.print-failed.webp" width="480" alt="app hc profit-and-loss.print-failed"></td>
</tr>
</table>

## hc / day-book.month

SSIM 0.5094

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/day-book.month.webp" width="480" alt="mock hc day-book.month"></td>
<td><img src="app-captures/hc/day-book.month.webp" width="480" alt="app hc day-book.month"></td>
</tr>
</table>

## hc / balance-sheet.provisional

SSIM 0.5160

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/balance-sheet.provisional.webp" width="480" alt="mock hc balance-sheet.provisional"></td>
<td><img src="app-captures/hc/balance-sheet.provisional.webp" width="480" alt="app hc balance-sheet.provisional"></td>
</tr>
</table>

## hc / masters.loading

SSIM 0.5166

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.loading.webp" width="480" alt="mock hc masters.loading"></td>
<td><img src="app-captures/hc/masters.loading.webp" width="480" alt="app hc masters.loading"></td>
</tr>
</table>

## hc / palette.entry

SSIM 0.5197

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/palette.entry.webp" width="480" alt="mock hc palette.entry"></td>
<td><img src="app-captures/hc/palette.entry.webp" width="480" alt="app hc palette.entry"></td>
</tr>
</table>

## hc / palette.ask

SSIM 0.5225

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/palette.ask.webp" width="480" alt="mock hc palette.ask"></td>
<td><img src="app-captures/hc/palette.ask.webp" width="480" alt="app hc palette.ask"></td>
</tr>
</table>

## hc / palette.voucher

SSIM 0.5312

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/palette.voucher.webp" width="480" alt="mock hc palette.voucher"></td>
<td><img src="app-captures/hc/palette.voucher.webp" width="480" alt="app hc palette.voucher"></td>
</tr>
</table>

## dark / home.new-tab

SSIM 0.5329

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/home.new-tab.webp" width="480" alt="mock dark home.new-tab"></td>
<td><img src="app-captures/dark/home.new-tab.webp" width="480" alt="app dark home.new-tab"></td>
</tr>
</table>

## light / home.new-tab

SSIM 0.5340

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/home.new-tab.webp" width="480" alt="mock light home.new-tab"></td>
<td><img src="app-captures/light/home.new-tab.webp" width="480" alt="app light home.new-tab"></td>
</tr>
</table>

## light / home.default

SSIM 0.5371

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/home.default.webp" width="480" alt="mock light home.default"></td>
<td><img src="app-captures/light/home.default.webp" width="480" alt="app light home.default"></td>
</tr>
</table>

## hc / palette.no-results

SSIM 0.5374

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/palette.no-results.webp" width="480" alt="mock hc palette.no-results"></td>
<td><img src="app-captures/hc/palette.no-results.webp" width="480" alt="app hc palette.no-results"></td>
</tr>
</table>

## dark / home.default

SSIM 0.5421

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/home.default.webp" width="480" alt="mock dark home.default"></td>
<td><img src="app-captures/dark/home.default.webp" width="480" alt="app dark home.default"></td>
</tr>
</table>

## hc / profit-and-loss.detailed

SSIM 0.5429

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.detailed.webp" width="480" alt="mock hc profit-and-loss.detailed"></td>
<td><img src="app-captures/hc/profit-and-loss.detailed.webp" width="480" alt="app hc profit-and-loss.detailed"></td>
</tr>
</table>

## hc / sales-invoice.validation-error

SSIM 0.5437

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.validation-error.webp" width="480" alt="mock hc sales-invoice.validation-error"></td>
<td><img src="app-captures/hc/sales-invoice.validation-error.webp" width="480" alt="app hc sales-invoice.validation-error"></td>
</tr>
</table>

## hc / ai-settings.cloud

SSIM 0.5451

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai-settings.cloud.webp" width="480" alt="mock hc ai-settings.cloud"></td>
<td><img src="app-captures/hc/ai-settings.cloud.webp" width="480" alt="app hc ai-settings.cloud"></td>
</tr>
</table>

## light / home.nudge-open

SSIM 0.5452

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/home.nudge-open.webp" width="480" alt="mock light home.nudge-open"></td>
<td><img src="app-captures/light/home.nudge-open.webp" width="480" alt="app light home.nudge-open"></td>
</tr>
</table>

## hc / bank.offline-ai

SSIM 0.5477

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/bank.offline-ai.webp" width="480" alt="mock hc bank.offline-ai"></td>
<td><img src="app-captures/hc/bank.offline-ai.webp" width="480" alt="app hc bank.offline-ai"></td>
</tr>
</table>

## hc / review.default

SSIM 0.5479

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/review.default.webp" width="480" alt="mock hc review.default"></td>
<td><img src="app-captures/hc/review.default.webp" width="480" alt="app hc review.default"></td>
</tr>
</table>

## hc / stock-summary.provisional

SSIM 0.5496

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/stock-summary.provisional.webp" width="480" alt="mock hc stock-summary.provisional"></td>
<td><img src="app-captures/hc/stock-summary.provisional.webp" width="480" alt="app hc stock-summary.provisional"></td>
</tr>
</table>

## hc / ai-settings.local

SSIM 0.5533

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai-settings.local.webp" width="480" alt="mock hc ai-settings.local"></td>
<td><img src="app-captures/hc/ai-settings.local.webp" width="480" alt="app hc ai-settings.local"></td>
</tr>
</table>

## dark / home.nudge-open

SSIM 0.5637

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/home.nudge-open.webp" width="480" alt="mock dark home.nudge-open"></td>
<td><img src="app-captures/dark/home.nudge-open.webp" width="480" alt="app dark home.nudge-open"></td>
</tr>
</table>

## hc / trial-balance.default

SSIM 0.5691

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/trial-balance.default.webp" width="480" alt="mock hc trial-balance.default"></td>
<td><img src="app-captures/hc/trial-balance.default.webp" width="480" alt="app hc trial-balance.default"></td>
</tr>
</table>

## dark / profit-and-loss.print-preview

SSIM 0.5716

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.print-preview.webp" width="480" alt="mock dark profit-and-loss.print-preview"></td>
<td><img src="app-captures/dark/profit-and-loss.print-preview.webp" width="480" alt="app dark profit-and-loss.print-preview"></td>
</tr>
</table>

## hc / import.imported

SSIM 0.5720

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.imported.webp" width="480" alt="mock hc import.imported"></td>
<td><img src="app-captures/hc/import.imported.webp" width="480" alt="app hc import.imported"></td>
</tr>
</table>

## hc / masters.ledgers

SSIM 0.5753

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.ledgers.webp" width="480" alt="mock hc masters.ledgers"></td>
<td><img src="app-captures/hc/masters.ledgers.webp" width="480" alt="app hc masters.ledgers"></td>
</tr>
</table>

## light / home.gst-unsupported

SSIM 0.5756

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/home.gst-unsupported.webp" width="480" alt="mock light home.gst-unsupported"></td>
<td><img src="app-captures/light/home.gst-unsupported.webp" width="480" alt="app light home.gst-unsupported"></td>
</tr>
</table>

## hc / import.no-company

SSIM 0.5782

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.no-company.webp" width="480" alt="mock hc import.no-company"></td>
<td><img src="app-captures/hc/import.no-company.webp" width="480" alt="app hc import.no-company"></td>
</tr>
</table>

## light / audit.default

SSIM 0.5783

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/audit.default.webp" width="480" alt="mock light audit.default"></td>
<td><img src="app-captures/light/audit.default.webp" width="480" alt="app light audit.default"></td>
</tr>
</table>

## dark / bank.default

SSIM 0.5784

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/bank.default.webp" width="480" alt="mock dark bank.default"></td>
<td><img src="app-captures/dark/bank.default.webp" width="480" alt="app dark bank.default"></td>
</tr>
</table>

## hc / home.ai-off

SSIM 0.5785

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/home.ai-off.webp" width="480" alt="mock hc home.ai-off"></td>
<td><img src="app-captures/hc/home.ai-off.webp" width="480" alt="app hc home.ai-off"></td>
</tr>
</table>

## dark / home.gst-unsupported

SSIM 0.5790

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/home.gst-unsupported.webp" width="480" alt="mock dark home.gst-unsupported"></td>
<td><img src="app-captures/dark/home.gst-unsupported.webp" width="480" alt="app dark home.gst-unsupported"></td>
</tr>
</table>

## hc / palette.search

SSIM 0.5813

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/palette.search.webp" width="480" alt="mock hc palette.search"></td>
<td><img src="app-captures/hc/palette.search.webp" width="480" alt="app hc palette.search"></td>
</tr>
</table>

## hc / profit-and-loss.print-preview

SSIM 0.5825

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.print-preview.webp" width="480" alt="mock hc profit-and-loss.print-preview"></td>
<td><img src="app-captures/hc/profit-and-loss.print-preview.webp" width="480" alt="app hc profit-and-loss.print-preview"></td>
</tr>
</table>

## hc / profit-and-loss.print-busy

SSIM 0.5839

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.print-busy.webp" width="480" alt="mock hc profit-and-loss.print-busy"></td>
<td><img src="app-captures/hc/profit-and-loss.print-busy.webp" width="480" alt="app hc profit-and-loss.print-busy"></td>
</tr>
</table>

## light / palette.entry

SSIM 0.5862

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/palette.entry.webp" width="480" alt="mock light palette.entry"></td>
<td><img src="app-captures/light/palette.entry.webp" width="480" alt="app light palette.entry"></td>
</tr>
</table>

## light / palette.ask

SSIM 0.5890

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/palette.ask.webp" width="480" alt="mock light palette.ask"></td>
<td><img src="app-captures/light/palette.ask.webp" width="480" alt="app light palette.ask"></td>
</tr>
</table>

## hc / profit-and-loss.print-saved

SSIM 0.5904

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.print-saved.webp" width="480" alt="mock hc profit-and-loss.print-saved"></td>
<td><img src="app-captures/hc/profit-and-loss.print-saved.webp" width="480" alt="app hc profit-and-loss.print-saved"></td>
</tr>
</table>

## hc / sales-invoice.item-list

SSIM 0.5916

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.item-list.webp" width="480" alt="mock hc sales-invoice.item-list"></td>
<td><img src="app-captures/hc/sales-invoice.item-list.webp" width="480" alt="app hc sales-invoice.item-list"></td>
</tr>
</table>

## hc / import.port-error

SSIM 0.5924

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.port-error.webp" width="480" alt="mock hc import.port-error"></td>
<td><img src="app-captures/hc/import.port-error.webp" width="480" alt="app hc import.port-error"></td>
</tr>
</table>

## hc / import.validation-error

SSIM 0.5934

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.validation-error.webp" width="480" alt="mock hc import.validation-error"></td>
<td><img src="app-captures/hc/import.validation-error.webp" width="480" alt="app hc import.validation-error"></td>
</tr>
</table>

## dark / audit.default

SSIM 0.5945

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/audit.default.webp" width="480" alt="mock dark audit.default"></td>
<td><img src="app-captures/dark/audit.default.webp" width="480" alt="app dark audit.default"></td>
</tr>
</table>

## hc / import.failed

SSIM 0.5947

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.failed.webp" width="480" alt="mock hc import.failed"></td>
<td><img src="app-captures/hc/import.failed.webp" width="480" alt="app hc import.failed"></td>
</tr>
</table>

## hc / import.refused

SSIM 0.5951

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.refused.webp" width="480" alt="mock hc import.refused"></td>
<td><img src="app-captures/hc/import.refused.webp" width="480" alt="app hc import.refused"></td>
</tr>
</table>

## hc / palette.default

SSIM 0.5970

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/palette.default.webp" width="480" alt="mock hc palette.default"></td>
<td><img src="app-captures/hc/palette.default.webp" width="480" alt="app hc palette.default"></td>
</tr>
</table>

## hc / sales-invoice.period-locked

SSIM 0.5974

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.period-locked.webp" width="480" alt="mock hc sales-invoice.period-locked"></td>
<td><img src="app-captures/hc/sales-invoice.period-locked.webp" width="480" alt="app hc sales-invoice.period-locked"></td>
</tr>
</table>

## light / palette.voucher

SSIM 0.5980

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/palette.voucher.webp" width="480" alt="mock light palette.voucher"></td>
<td><img src="app-captures/light/palette.voucher.webp" width="480" alt="app light palette.voucher"></td>
</tr>
</table>

## light / palette.no-results

SSIM 0.5990

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/palette.no-results.webp" width="480" alt="mock light palette.no-results"></td>
<td><img src="app-captures/light/palette.no-results.webp" width="480" alt="app light palette.no-results"></td>
</tr>
</table>

## hc / keys.standard

SSIM 0.6004

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/keys.standard.webp" width="480" alt="mock hc keys.standard"></td>
<td><img src="app-captures/hc/keys.standard.webp" width="480" alt="app hc keys.standard"></td>
</tr>
</table>

## hc / purchase.validation-error

SSIM 0.6011

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/purchase.validation-error.webp" width="480" alt="mock hc purchase.validation-error"></td>
<td><img src="app-captures/hc/purchase.validation-error.webp" width="480" alt="app hc purchase.validation-error"></td>
</tr>
</table>

## hc / balance-sheet.detailed

SSIM 0.6021

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/balance-sheet.detailed.webp" width="480" alt="mock hc balance-sheet.detailed"></td>
<td><img src="app-captures/hc/balance-sheet.detailed.webp" width="480" alt="app hc balance-sheet.detailed"></td>
</tr>
</table>

## light / masters.ledgers

SSIM 0.6024

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.ledgers.webp" width="480" alt="mock light masters.ledgers"></td>
<td><img src="app-captures/light/masters.ledgers.webp" width="480" alt="app light masters.ledgers"></td>
</tr>
</table>

## light / review.default

SSIM 0.6026

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/review.default.webp" width="480" alt="mock light review.default"></td>
<td><img src="app-captures/light/review.default.webp" width="480" alt="app light review.default"></td>
</tr>
</table>

## hc / gst.gstr1-correction

SSIM 0.6033

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.gstr1-correction.webp" width="480" alt="mock hc gst.gstr1-correction"></td>
<td><img src="app-captures/hc/gst.gstr1-correction.webp" width="480" alt="app hc gst.gstr1-correction"></td>
</tr>
</table>

## hc / gst.gstr1

SSIM 0.6034

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.gstr1.webp" width="480" alt="mock hc gst.gstr1"></td>
<td><img src="app-captures/hc/gst.gstr1.webp" width="480" alt="app hc gst.gstr1"></td>
</tr>
</table>

## hc / purchase-bill.offline-ai

SSIM 0.6035

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/purchase-bill.offline-ai.webp" width="480" alt="mock hc purchase-bill.offline-ai"></td>
<td><img src="app-captures/hc/purchase-bill.offline-ai.webp" width="480" alt="app hc purchase-bill.offline-ai"></td>
</tr>
</table>

## dark / review.default

SSIM 0.6039

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/review.default.webp" width="480" alt="mock dark review.default"></td>
<td><img src="app-captures/dark/review.default.webp" width="480" alt="app dark review.default"></td>
</tr>
</table>

## light / sales-invoice.save-confirm

SSIM 0.6043

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.save-confirm.webp" width="480" alt="mock light sales-invoice.save-confirm"></td>
<td><img src="app-captures/light/sales-invoice.save-confirm.webp" width="480" alt="app light sales-invoice.save-confirm"></td>
</tr>
</table>

## light / bank.default

SSIM 0.6059

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/bank.default.webp" width="480" alt="mock light bank.default"></td>
<td><img src="app-captures/light/bank.default.webp" width="480" alt="app light bank.default"></td>
</tr>
</table>

## dark / bank.offline-ai

SSIM 0.6067

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/bank.offline-ai.webp" width="480" alt="mock dark bank.offline-ai"></td>
<td><img src="app-captures/dark/bank.offline-ai.webp" width="480" alt="app dark bank.offline-ai"></td>
</tr>
</table>

## hc / masters.items

SSIM 0.6068

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.items.webp" width="480" alt="mock hc masters.items"></td>
<td><img src="app-captures/hc/masters.items.webp" width="480" alt="app hc masters.items"></td>
</tr>
</table>

## hc / keys.classic

SSIM 0.6069

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/keys.classic.webp" width="480" alt="mock hc keys.classic"></td>
<td><img src="app-captures/hc/keys.classic.webp" width="480" alt="app hc keys.classic"></td>
</tr>
</table>

## hc / balance-sheet.default

SSIM 0.6087

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/balance-sheet.default.webp" width="480" alt="mock hc balance-sheet.default"></td>
<td><img src="app-captures/hc/balance-sheet.default.webp" width="480" alt="app hc balance-sheet.default"></td>
</tr>
</table>

## light / balance-sheet.provisional

SSIM 0.6118

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/balance-sheet.provisional.webp" width="480" alt="mock light balance-sheet.provisional"></td>
<td><img src="app-captures/light/balance-sheet.provisional.webp" width="480" alt="app light balance-sheet.provisional"></td>
</tr>
</table>

## light / ai-settings.cloud

SSIM 0.6137

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai-settings.cloud.webp" width="480" alt="mock light ai-settings.cloud"></td>
<td><img src="app-captures/light/ai-settings.cloud.webp" width="480" alt="app light ai-settings.cloud"></td>
</tr>
</table>

## hc / masters.no-match

SSIM 0.6139

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.no-match.webp" width="480" alt="mock hc masters.no-match"></td>
<td><img src="app-captures/hc/masters.no-match.webp" width="480" alt="app hc masters.no-match"></td>
</tr>
</table>

## light / day-book.month

SSIM 0.6157

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/day-book.month.webp" width="480" alt="mock light day-book.month"></td>
<td><img src="app-captures/light/day-book.month.webp" width="480" alt="app light day-book.month"></td>
</tr>
</table>

## dark / palette.entry

SSIM 0.6165

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/palette.entry.webp" width="480" alt="mock dark palette.entry"></td>
<td><img src="app-captures/dark/palette.entry.webp" width="480" alt="app dark palette.entry"></td>
</tr>
</table>

## hc / sales-invoice.gst-inter-state

SSIM 0.6165

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.gst-inter-state.webp" width="480" alt="mock hc sales-invoice.gst-inter-state"></td>
<td><img src="app-captures/hc/sales-invoice.gst-inter-state.webp" width="480" alt="app hc sales-invoice.gst-inter-state"></td>
</tr>
</table>

## light / trial-balance.default

SSIM 0.6168

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/trial-balance.default.webp" width="480" alt="mock light trial-balance.default"></td>
<td><img src="app-captures/light/trial-balance.default.webp" width="480" alt="app light trial-balance.default"></td>
</tr>
</table>

## dark / sales-invoice.save-confirm

SSIM 0.6184

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.save-confirm.webp" width="480" alt="mock dark sales-invoice.save-confirm"></td>
<td><img src="app-captures/dark/sales-invoice.save-confirm.webp" width="480" alt="app dark sales-invoice.save-confirm"></td>
</tr>
</table>

## light / sales-invoice.period-locked

SSIM 0.6188

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.period-locked.webp" width="480" alt="mock light sales-invoice.period-locked"></td>
<td><img src="app-captures/light/sales-invoice.period-locked.webp" width="480" alt="app light sales-invoice.period-locked"></td>
</tr>
</table>

## dark / palette.ask

SSIM 0.6191

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/palette.ask.webp" width="480" alt="mock dark palette.ask"></td>
<td><img src="app-captures/dark/palette.ask.webp" width="480" alt="app dark palette.ask"></td>
</tr>
</table>

## hc / sales-invoice.default

SSIM 0.6222

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.default.webp" width="480" alt="mock hc sales-invoice.default"></td>
<td><img src="app-captures/hc/sales-invoice.default.webp" width="480" alt="app hc sales-invoice.default"></td>
</tr>
</table>

## dark / palette.no-results

SSIM 0.6241

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/palette.no-results.webp" width="480" alt="mock dark palette.no-results"></td>
<td><img src="app-captures/dark/palette.no-results.webp" width="480" alt="app dark palette.no-results"></td>
</tr>
</table>

## dark / ai-settings.cloud

SSIM 0.6247

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai-settings.cloud.webp" width="480" alt="mock dark ai-settings.cloud"></td>
<td><img src="app-captures/dark/ai-settings.cloud.webp" width="480" alt="app dark ai-settings.cloud"></td>
</tr>
</table>

## light / profit-and-loss.detailed

SSIM 0.6250

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.detailed.webp" width="480" alt="mock light profit-and-loss.detailed"></td>
<td><img src="app-captures/light/profit-and-loss.detailed.webp" width="480" alt="app light profit-and-loss.detailed"></td>
</tr>
</table>

## dark / day-book.month

SSIM 0.6269

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/day-book.month.webp" width="480" alt="mock dark day-book.month"></td>
<td><img src="app-captures/dark/day-book.month.webp" width="480" alt="app dark day-book.month"></td>
</tr>
</table>

## light / bank.offline-ai

SSIM 0.6272

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/bank.offline-ai.webp" width="480" alt="mock light bank.offline-ai"></td>
<td><img src="app-captures/light/bank.offline-ai.webp" width="480" alt="app light bank.offline-ai"></td>
</tr>
</table>

## dark / palette.voucher

SSIM 0.6284

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/palette.voucher.webp" width="480" alt="mock dark palette.voucher"></td>
<td><img src="app-captures/dark/palette.voucher.webp" width="480" alt="app dark palette.voucher"></td>
</tr>
</table>

## light / sales-invoice.default

SSIM 0.6296

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.default.webp" width="480" alt="mock light sales-invoice.default"></td>
<td><img src="app-captures/light/sales-invoice.default.webp" width="480" alt="app light sales-invoice.default"></td>
</tr>
</table>

## light / palette.search

SSIM 0.6303

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/palette.search.webp" width="480" alt="mock light palette.search"></td>
<td><img src="app-captures/light/palette.search.webp" width="480" alt="app light palette.search"></td>
</tr>
</table>

## light / sales-invoice.item-list

SSIM 0.6306

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.item-list.webp" width="480" alt="mock light sales-invoice.item-list"></td>
<td><img src="app-captures/light/sales-invoice.item-list.webp" width="480" alt="app light sales-invoice.item-list"></td>
</tr>
</table>

## hc / import.running

SSIM 0.6307

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.running.webp" width="480" alt="mock hc import.running"></td>
<td><img src="app-captures/hc/import.running.webp" width="480" alt="app hc import.running"></td>
</tr>
</table>

## dark / masters.ledgers

SSIM 0.6330

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.ledgers.webp" width="480" alt="mock dark masters.ledgers"></td>
<td><img src="app-captures/dark/masters.ledgers.webp" width="480" alt="app dark masters.ledgers"></td>
</tr>
</table>

## hc / palette.commands

SSIM 0.6333

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/palette.commands.webp" width="480" alt="mock hc palette.commands"></td>
<td><img src="app-captures/hc/palette.commands.webp" width="480" alt="app hc palette.commands"></td>
</tr>
</table>

## light / sales-invoice.gst-inter-state

SSIM 0.6339

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.gst-inter-state.webp" width="480" alt="mock light sales-invoice.gst-inter-state"></td>
<td><img src="app-captures/light/sales-invoice.gst-inter-state.webp" width="480" alt="app light sales-invoice.gst-inter-state"></td>
</tr>
</table>

## hc / ai-settings.off

SSIM 0.6367

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai-settings.off.webp" width="480" alt="mock hc ai-settings.off"></td>
<td><img src="app-captures/hc/ai-settings.off.webp" width="480" alt="app hc ai-settings.off"></td>
</tr>
</table>

## light / sales-invoice.validation-error

SSIM 0.6367

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.validation-error.webp" width="480" alt="mock light sales-invoice.validation-error"></td>
<td><img src="app-captures/light/sales-invoice.validation-error.webp" width="480" alt="app light sales-invoice.validation-error"></td>
</tr>
</table>

## dark / sales-invoice.period-locked

SSIM 0.6384

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.period-locked.webp" width="480" alt="mock dark sales-invoice.period-locked"></td>
<td><img src="app-captures/dark/sales-invoice.period-locked.webp" width="480" alt="app dark sales-invoice.period-locked"></td>
</tr>
</table>

## hc / profit-and-loss.default

SSIM 0.6387

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.default.webp" width="480" alt="mock hc profit-and-loss.default"></td>
<td><img src="app-captures/hc/profit-and-loss.default.webp" width="480" alt="app hc profit-and-loss.default"></td>
</tr>
</table>

## dark / sales-invoice.item-list

SSIM 0.6394

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.item-list.webp" width="480" alt="mock dark sales-invoice.item-list"></td>
<td><img src="app-captures/dark/sales-invoice.item-list.webp" width="480" alt="app dark sales-invoice.item-list"></td>
</tr>
</table>

## light / profit-and-loss.provisional

SSIM 0.6412

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.provisional.webp" width="480" alt="mock light profit-and-loss.provisional"></td>
<td><img src="app-captures/light/profit-and-loss.provisional.webp" width="480" alt="app light profit-and-loss.provisional"></td>
</tr>
</table>

## dark / sales-invoice.validation-error

SSIM 0.6421

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.validation-error.webp" width="480" alt="mock dark sales-invoice.validation-error"></td>
<td><img src="app-captures/dark/sales-invoice.validation-error.webp" width="480" alt="app dark sales-invoice.validation-error"></td>
</tr>
</table>

## dark / profit-and-loss.provisional

SSIM 0.6446

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.provisional.webp" width="480" alt="mock dark profit-and-loss.provisional"></td>
<td><img src="app-captures/dark/profit-and-loss.provisional.webp" width="480" alt="app dark profit-and-loss.provisional"></td>
</tr>
</table>

## dark / balance-sheet.provisional

SSIM 0.6449

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/balance-sheet.provisional.webp" width="480" alt="mock dark balance-sheet.provisional"></td>
<td><img src="app-captures/dark/balance-sheet.provisional.webp" width="480" alt="app dark balance-sheet.provisional"></td>
</tr>
</table>

## hc / purchase.default

SSIM 0.6458

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/purchase.default.webp" width="480" alt="mock hc purchase.default"></td>
<td><img src="app-captures/hc/purchase.default.webp" width="480" alt="app hc purchase.default"></td>
</tr>
</table>

## hc / sales-invoice.b2c

SSIM 0.6468

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.b2c.webp" width="480" alt="mock hc sales-invoice.b2c"></td>
<td><img src="app-captures/hc/sales-invoice.b2c.webp" width="480" alt="app hc sales-invoice.b2c"></td>
</tr>
</table>

## hc / import.default

SSIM 0.6475

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.default.webp" width="480" alt="mock hc import.default"></td>
<td><img src="app-captures/hc/import.default.webp" width="480" alt="app hc import.default"></td>
</tr>
</table>

## light / profit-and-loss.print-preview

SSIM 0.6486

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.print-preview.webp" width="480" alt="mock light profit-and-loss.print-preview"></td>
<td><img src="app-captures/light/profit-and-loss.print-preview.webp" width="480" alt="app light profit-and-loss.print-preview"></td>
</tr>
</table>

## dark / sales-invoice.default

SSIM 0.6502

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.default.webp" width="480" alt="mock dark sales-invoice.default"></td>
<td><img src="app-captures/dark/sales-invoice.default.webp" width="480" alt="app dark sales-invoice.default"></td>
</tr>
</table>

## light / profit-and-loss.print-busy

SSIM 0.6505

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.print-busy.webp" width="480" alt="mock light profit-and-loss.print-busy"></td>
<td><img src="app-captures/light/profit-and-loss.print-busy.webp" width="480" alt="app light profit-and-loss.print-busy"></td>
</tr>
</table>

## dark / trial-balance.default

SSIM 0.6510

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/trial-balance.default.webp" width="480" alt="mock dark trial-balance.default"></td>
<td><img src="app-captures/dark/trial-balance.default.webp" width="480" alt="app dark trial-balance.default"></td>
</tr>
</table>

## dark / palette.search

SSIM 0.6519

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/palette.search.webp" width="480" alt="mock dark palette.search"></td>
<td><img src="app-captures/dark/palette.search.webp" width="480" alt="app dark palette.search"></td>
</tr>
</table>

## dark / purchase-bill.offline-ai

SSIM 0.6543

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/purchase-bill.offline-ai.webp" width="480" alt="mock dark purchase-bill.offline-ai"></td>
<td><img src="app-captures/dark/purchase-bill.offline-ai.webp" width="480" alt="app dark purchase-bill.offline-ai"></td>
</tr>
</table>

## light / profit-and-loss.print-failed

SSIM 0.6543

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.print-failed.webp" width="480" alt="mock light profit-and-loss.print-failed"></td>
<td><img src="app-captures/light/profit-and-loss.print-failed.webp" width="480" alt="app light profit-and-loss.print-failed"></td>
</tr>
</table>

## dark / profit-and-loss.detailed

SSIM 0.6545

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.detailed.webp" width="480" alt="mock dark profit-and-loss.detailed"></td>
<td><img src="app-captures/dark/profit-and-loss.detailed.webp" width="480" alt="app dark profit-and-loss.detailed"></td>
</tr>
</table>

## light / keys.standard

SSIM 0.6546

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/keys.standard.webp" width="480" alt="mock light keys.standard"></td>
<td><img src="app-captures/light/keys.standard.webp" width="480" alt="app light keys.standard"></td>
</tr>
</table>

## light / balance-sheet.detailed

SSIM 0.6556

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/balance-sheet.detailed.webp" width="480" alt="mock light balance-sheet.detailed"></td>
<td><img src="app-captures/light/balance-sheet.detailed.webp" width="480" alt="app light balance-sheet.detailed"></td>
</tr>
</table>

## dark / keys.standard

SSIM 0.6557

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/keys.standard.webp" width="480" alt="mock dark keys.standard"></td>
<td><img src="app-captures/dark/keys.standard.webp" width="480" alt="app dark keys.standard"></td>
</tr>
</table>

## dark / sales-invoice.gst-inter-state

SSIM 0.6557

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.gst-inter-state.webp" width="480" alt="mock dark sales-invoice.gst-inter-state"></td>
<td><img src="app-captures/dark/sales-invoice.gst-inter-state.webp" width="480" alt="app dark sales-invoice.gst-inter-state"></td>
</tr>
</table>

## light / palette.default

SSIM 0.6565

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/palette.default.webp" width="480" alt="mock light palette.default"></td>
<td><img src="app-captures/light/palette.default.webp" width="480" alt="app light palette.default"></td>
</tr>
</table>

## dark / profit-and-loss.print-failed

SSIM 0.6569

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.print-failed.webp" width="480" alt="mock dark profit-and-loss.print-failed"></td>
<td><img src="app-captures/dark/profit-and-loss.print-failed.webp" width="480" alt="app dark profit-and-loss.print-failed"></td>
</tr>
</table>

## hc / payment.overdraft

SSIM 0.6571

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/payment.overdraft.webp" width="480" alt="mock hc payment.overdraft"></td>
<td><img src="app-captures/hc/payment.overdraft.webp" width="480" alt="app hc payment.overdraft"></td>
</tr>
</table>

## hc / credit-note.validation-error

SSIM 0.6581

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/credit-note.validation-error.webp" width="480" alt="mock hc credit-note.validation-error"></td>
<td><img src="app-captures/hc/credit-note.validation-error.webp" width="480" alt="app hc credit-note.validation-error"></td>
</tr>
</table>

## hc / bank.brs

SSIM 0.6585

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/bank.brs.webp" width="480" alt="mock hc bank.brs"></td>
<td><img src="app-captures/hc/bank.brs.webp" width="480" alt="app hc bank.brs"></td>
</tr>
</table>

## dark / stock-summary.provisional

SSIM 0.6590

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/stock-summary.provisional.webp" width="480" alt="mock dark stock-summary.provisional"></td>
<td><img src="app-captures/dark/stock-summary.provisional.webp" width="480" alt="app dark stock-summary.provisional"></td>
</tr>
</table>

## light / stock-summary.provisional

SSIM 0.6591

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/stock-summary.provisional.webp" width="480" alt="mock light stock-summary.provisional"></td>
<td><img src="app-captures/light/stock-summary.provisional.webp" width="480" alt="app light stock-summary.provisional"></td>
</tr>
</table>

## hc / gst.gstr3b

SSIM 0.6594

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.gstr3b.webp" width="480" alt="mock hc gst.gstr3b"></td>
<td><img src="app-captures/hc/gst.gstr3b.webp" width="480" alt="app hc gst.gstr3b"></td>
</tr>
</table>

## hc / sales-invoice.saved

SSIM 0.6601

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.saved.webp" width="480" alt="mock hc sales-invoice.saved"></td>
<td><img src="app-captures/hc/sales-invoice.saved.webp" width="480" alt="app hc sales-invoice.saved"></td>
</tr>
</table>

## light / ai-settings.local

SSIM 0.6612

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai-settings.local.webp" width="480" alt="mock light ai-settings.local"></td>
<td><img src="app-captures/light/ai-settings.local.webp" width="480" alt="app light ai-settings.local"></td>
</tr>
</table>

## light / sales-invoice.b2c

SSIM 0.6613

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.b2c.webp" width="480" alt="mock light sales-invoice.b2c"></td>
<td><img src="app-captures/light/sales-invoice.b2c.webp" width="480" alt="app light sales-invoice.b2c"></td>
</tr>
</table>

## hc / debit-note.validation-error

SSIM 0.6618

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/debit-note.validation-error.webp" width="480" alt="mock hc debit-note.validation-error"></td>
<td><img src="app-captures/hc/debit-note.validation-error.webp" width="480" alt="app hc debit-note.validation-error"></td>
</tr>
</table>

## light / balance-sheet.default

SSIM 0.6633

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/balance-sheet.default.webp" width="480" alt="mock light balance-sheet.default"></td>
<td><img src="app-captures/light/balance-sheet.default.webp" width="480" alt="app light balance-sheet.default"></td>
</tr>
</table>

## dark / profit-and-loss.print-busy

SSIM 0.6646

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.print-busy.webp" width="480" alt="mock dark profit-and-loss.print-busy"></td>
<td><img src="app-captures/dark/profit-and-loss.print-busy.webp" width="480" alt="app dark profit-and-loss.print-busy"></td>
</tr>
</table>

## hc / balance-sheet.voucher

SSIM 0.6647

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/balance-sheet.voucher.webp" width="480" alt="mock hc balance-sheet.voucher"></td>
<td><img src="app-captures/hc/balance-sheet.voucher.webp" width="480" alt="app hc balance-sheet.voucher"></td>
</tr>
</table>

## light / keys.classic

SSIM 0.6648

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/keys.classic.webp" width="480" alt="mock light keys.classic"></td>
<td><img src="app-captures/light/keys.classic.webp" width="480" alt="app light keys.classic"></td>
</tr>
</table>

## dark / keys.classic

SSIM 0.6651

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/keys.classic.webp" width="480" alt="mock dark keys.classic"></td>
<td><img src="app-captures/dark/keys.classic.webp" width="480" alt="app dark keys.classic"></td>
</tr>
</table>

## hc / balance-sheet.ledger

SSIM 0.6653

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/balance-sheet.ledger.webp" width="480" alt="mock hc balance-sheet.ledger"></td>
<td><img src="app-captures/hc/balance-sheet.ledger.webp" width="480" alt="app hc balance-sheet.ledger"></td>
</tr>
</table>

## dark / ai-settings.local

SSIM 0.6668

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai-settings.local.webp" width="480" alt="mock dark ai-settings.local"></td>
<td><img src="app-captures/dark/ai-settings.local.webp" width="480" alt="app dark ai-settings.local"></td>
</tr>
</table>

## dark / home.ai-off

SSIM 0.6673

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/home.ai-off.webp" width="480" alt="mock dark home.ai-off"></td>
<td><img src="app-captures/dark/home.ai-off.webp" width="480" alt="app dark home.ai-off"></td>
</tr>
</table>

## light / home.ai-off

SSIM 0.6705

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/home.ai-off.webp" width="480" alt="mock light home.ai-off"></td>
<td><img src="app-captures/light/home.ai-off.webp" width="480" alt="app light home.ai-off"></td>
</tr>
</table>

## dark / import.no-company

SSIM 0.6737

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.no-company.webp" width="480" alt="mock dark import.no-company"></td>
<td><img src="app-captures/dark/import.no-company.webp" width="480" alt="app dark import.no-company"></td>
</tr>
</table>

## light / gst.gstr1

SSIM 0.6737

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.gstr1.webp" width="480" alt="mock light gst.gstr1"></td>
<td><img src="app-captures/light/gst.gstr1.webp" width="480" alt="app light gst.gstr1"></td>
</tr>
</table>

## hc / outstanding.receivables

SSIM 0.6741

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/outstanding.receivables.webp" width="480" alt="mock hc outstanding.receivables"></td>
<td><img src="app-captures/hc/outstanding.receivables.webp" width="480" alt="app hc outstanding.receivables"></td>
</tr>
</table>

## dark / palette.default

SSIM 0.6744

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/palette.default.webp" width="480" alt="mock dark palette.default"></td>
<td><img src="app-captures/dark/palette.default.webp" width="480" alt="app dark palette.default"></td>
</tr>
</table>

## light / palette.commands

SSIM 0.6771

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/palette.commands.webp" width="480" alt="mock light palette.commands"></td>
<td><img src="app-captures/light/palette.commands.webp" width="480" alt="app light palette.commands"></td>
</tr>
</table>

## dark / gst.gstr1

SSIM 0.6781

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.gstr1.webp" width="480" alt="mock dark gst.gstr1"></td>
<td><img src="app-captures/dark/gst.gstr1.webp" width="480" alt="app dark gst.gstr1"></td>
</tr>
</table>

## light / import.no-company

SSIM 0.6803

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.no-company.webp" width="480" alt="mock light import.no-company"></td>
<td><img src="app-captures/light/import.no-company.webp" width="480" alt="app light import.no-company"></td>
</tr>
</table>

## dark / import.refused

SSIM 0.6807

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.refused.webp" width="480" alt="mock dark import.refused"></td>
<td><img src="app-captures/dark/import.refused.webp" width="480" alt="app dark import.refused"></td>
</tr>
</table>

## light / sales-invoice.saved

SSIM 0.6811

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.saved.webp" width="480" alt="mock light sales-invoice.saved"></td>
<td><img src="app-captures/light/sales-invoice.saved.webp" width="480" alt="app light sales-invoice.saved"></td>
</tr>
</table>

## dark / sales-invoice.b2c

SSIM 0.6815

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.b2c.webp" width="480" alt="mock dark sales-invoice.b2c"></td>
<td><img src="app-captures/dark/sales-invoice.b2c.webp" width="480" alt="app dark sales-invoice.b2c"></td>
</tr>
</table>

## light / import.refused

SSIM 0.6820

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.refused.webp" width="480" alt="mock light import.refused"></td>
<td><img src="app-captures/light/import.refused.webp" width="480" alt="app light import.refused"></td>
</tr>
</table>

## dark / import.failed

SSIM 0.6823

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.failed.webp" width="480" alt="mock dark import.failed"></td>
<td><img src="app-captures/dark/import.failed.webp" width="480" alt="app dark import.failed"></td>
</tr>
</table>

## dark / import.imported

SSIM 0.6827

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.imported.webp" width="480" alt="mock dark import.imported"></td>
<td><img src="app-captures/dark/import.imported.webp" width="480" alt="app dark import.imported"></td>
</tr>
</table>

## hc / journal.validation-error

SSIM 0.6831

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/journal.validation-error.webp" width="480" alt="mock hc journal.validation-error"></td>
<td><img src="app-captures/hc/journal.validation-error.webp" width="480" alt="app hc journal.validation-error"></td>
</tr>
</table>

## light / import.failed

SSIM 0.6833

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.failed.webp" width="480" alt="mock light import.failed"></td>
<td><img src="app-captures/light/import.failed.webp" width="480" alt="app light import.failed"></td>
</tr>
</table>

## light / gst.gstr1-correction

SSIM 0.6846

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.gstr1-correction.webp" width="480" alt="mock light gst.gstr1-correction"></td>
<td><img src="app-captures/light/gst.gstr1-correction.webp" width="480" alt="app light gst.gstr1-correction"></td>
</tr>
</table>

## dark / import.port-error

SSIM 0.6857

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.port-error.webp" width="480" alt="mock dark import.port-error"></td>
<td><img src="app-captures/dark/import.port-error.webp" width="480" alt="app dark import.port-error"></td>
</tr>
</table>

## light / ai-settings.off

SSIM 0.6872

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai-settings.off.webp" width="480" alt="mock light ai-settings.off"></td>
<td><img src="app-captures/light/ai-settings.off.webp" width="480" alt="app light ai-settings.off"></td>
</tr>
</table>

## light / import.imported

SSIM 0.6872

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.imported.webp" width="480" alt="mock light import.imported"></td>
<td><img src="app-captures/light/import.imported.webp" width="480" alt="app light import.imported"></td>
</tr>
</table>

## dark / import.validation-error

SSIM 0.6872

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.validation-error.webp" width="480" alt="mock dark import.validation-error"></td>
<td><img src="app-captures/dark/import.validation-error.webp" width="480" alt="app dark import.validation-error"></td>
</tr>
</table>

## dark / gst.gstr1-correction

SSIM 0.6893

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.gstr1-correction.webp" width="480" alt="mock dark gst.gstr1-correction"></td>
<td><img src="app-captures/dark/gst.gstr1-correction.webp" width="480" alt="app dark gst.gstr1-correction"></td>
</tr>
</table>

## hc / receipt.default

SSIM 0.6896

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/receipt.default.webp" width="480" alt="mock hc receipt.default"></td>
<td><img src="app-captures/hc/receipt.default.webp" width="480" alt="app hc receipt.default"></td>
</tr>
</table>

## light / outstanding.receivables

SSIM 0.6902

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/outstanding.receivables.webp" width="480" alt="mock light outstanding.receivables"></td>
<td><img src="app-captures/light/outstanding.receivables.webp" width="480" alt="app light outstanding.receivables"></td>
</tr>
</table>

## dark / ai-settings.off

SSIM 0.6917

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai-settings.off.webp" width="480" alt="mock dark ai-settings.off"></td>
<td><img src="app-captures/dark/ai-settings.off.webp" width="480" alt="app dark ai-settings.off"></td>
</tr>
</table>

## light / import.port-error

SSIM 0.6918

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.port-error.webp" width="480" alt="mock light import.port-error"></td>
<td><img src="app-captures/light/import.port-error.webp" width="480" alt="app light import.port-error"></td>
</tr>
</table>

## dark / masters.no-match

SSIM 0.6924

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.no-match.webp" width="480" alt="mock dark masters.no-match"></td>
<td><img src="app-captures/dark/masters.no-match.webp" width="480" alt="app dark masters.no-match"></td>
</tr>
</table>

## hc / payment.bill-wise

SSIM 0.6925

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/payment.bill-wise.webp" width="480" alt="mock hc payment.bill-wise"></td>
<td><img src="app-captures/hc/payment.bill-wise.webp" width="480" alt="app hc payment.bill-wise"></td>
</tr>
</table>

## light / import.validation-error

SSIM 0.6931

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.validation-error.webp" width="480" alt="mock light import.validation-error"></td>
<td><img src="app-captures/light/import.validation-error.webp" width="480" alt="app light import.validation-error"></td>
</tr>
</table>

## dark / balance-sheet.detailed

SSIM 0.6955

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/balance-sheet.detailed.webp" width="480" alt="mock dark balance-sheet.detailed"></td>
<td><img src="app-captures/dark/balance-sheet.detailed.webp" width="480" alt="app dark balance-sheet.detailed"></td>
</tr>
</table>

## dark / sales-invoice.saved

SSIM 0.6963

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.saved.webp" width="480" alt="mock dark sales-invoice.saved"></td>
<td><img src="app-captures/dark/sales-invoice.saved.webp" width="480" alt="app dark sales-invoice.saved"></td>
</tr>
</table>

## dark / import.running

SSIM 0.6974

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.running.webp" width="480" alt="mock dark import.running"></td>
<td><img src="app-captures/dark/import.running.webp" width="480" alt="app dark import.running"></td>
</tr>
</table>

## dark / import.default

SSIM 0.6977

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.default.webp" width="480" alt="mock dark import.default"></td>
<td><img src="app-captures/dark/import.default.webp" width="480" alt="app dark import.default"></td>
</tr>
</table>

## dark / palette.commands

SSIM 0.6980

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/palette.commands.webp" width="480" alt="mock dark palette.commands"></td>
<td><img src="app-captures/dark/palette.commands.webp" width="480" alt="app dark palette.commands"></td>
</tr>
</table>

## light / masters.no-match

SSIM 0.6987

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.no-match.webp" width="480" alt="mock light masters.no-match"></td>
<td><img src="app-captures/light/masters.no-match.webp" width="480" alt="app light masters.no-match"></td>
</tr>
</table>

## light / profit-and-loss.print-saved

SSIM 0.6989

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.print-saved.webp" width="480" alt="mock light profit-and-loss.print-saved"></td>
<td><img src="app-captures/light/profit-and-loss.print-saved.webp" width="480" alt="app light profit-and-loss.print-saved"></td>
</tr>
</table>

## light / profit-and-loss.default

SSIM 0.6992

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.default.webp" width="480" alt="mock light profit-and-loss.default"></td>
<td><img src="app-captures/light/profit-and-loss.default.webp" width="480" alt="app light profit-and-loss.default"></td>
</tr>
</table>

## dark / balance-sheet.default

SSIM 0.7009

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/balance-sheet.default.webp" width="480" alt="mock dark balance-sheet.default"></td>
<td><img src="app-captures/dark/balance-sheet.default.webp" width="480" alt="app dark balance-sheet.default"></td>
</tr>
</table>

## light / gst.gstr3b

SSIM 0.7014

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.gstr3b.webp" width="480" alt="mock light gst.gstr3b"></td>
<td><img src="app-captures/light/gst.gstr3b.webp" width="480" alt="app light gst.gstr3b"></td>
</tr>
</table>

## light / import.running

SSIM 0.7014

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.running.webp" width="480" alt="mock light import.running"></td>
<td><img src="app-captures/light/import.running.webp" width="480" alt="app light import.running"></td>
</tr>
</table>

## dark / profit-and-loss.print-saved

SSIM 0.7022

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.print-saved.webp" width="480" alt="mock dark profit-and-loss.print-saved"></td>
<td><img src="app-captures/dark/profit-and-loss.print-saved.webp" width="480" alt="app dark profit-and-loss.print-saved"></td>
</tr>
</table>

## dark / outstanding.receivables

SSIM 0.7027

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/outstanding.receivables.webp" width="480" alt="mock dark outstanding.receivables"></td>
<td><img src="app-captures/dark/outstanding.receivables.webp" width="480" alt="app dark outstanding.receivables"></td>
</tr>
</table>

## hc / credit-note.default

SSIM 0.7034

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/credit-note.default.webp" width="480" alt="mock hc credit-note.default"></td>
<td><img src="app-captures/hc/credit-note.default.webp" width="480" alt="app hc credit-note.default"></td>
</tr>
</table>

## light / import.default

SSIM 0.7051

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.default.webp" width="480" alt="mock light import.default"></td>
<td><img src="app-captures/light/import.default.webp" width="480" alt="app light import.default"></td>
</tr>
</table>

## light / purchase.default

SSIM 0.7062

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/purchase.default.webp" width="480" alt="mock light purchase.default"></td>
<td><img src="app-captures/light/purchase.default.webp" width="480" alt="app light purchase.default"></td>
</tr>
</table>

## hc / balance-sheet.group

SSIM 0.7072

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/balance-sheet.group.webp" width="480" alt="mock hc balance-sheet.group"></td>
<td><img src="app-captures/hc/balance-sheet.group.webp" width="480" alt="app hc balance-sheet.group"></td>
</tr>
</table>

## light / purchase.validation-error

SSIM 0.7079

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/purchase.validation-error.webp" width="480" alt="mock light purchase.validation-error"></td>
<td><img src="app-captures/light/purchase.validation-error.webp" width="480" alt="app light purchase.validation-error"></td>
</tr>
</table>

## dark / gst.gstr3b

SSIM 0.7083

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.gstr3b.webp" width="480" alt="mock dark gst.gstr3b"></td>
<td><img src="app-captures/dark/gst.gstr3b.webp" width="480" alt="app dark gst.gstr3b"></td>
</tr>
</table>

## dark / purchase.validation-error

SSIM 0.7085

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/purchase.validation-error.webp" width="480" alt="mock dark purchase.validation-error"></td>
<td><img src="app-captures/dark/purchase.validation-error.webp" width="480" alt="app dark purchase.validation-error"></td>
</tr>
</table>

## hc / gst.recon

SSIM 0.7086

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.recon.webp" width="480" alt="mock hc gst.recon"></td>
<td><img src="app-captures/hc/gst.recon.webp" width="480" alt="app hc gst.recon"></td>
</tr>
</table>

## hc / setup.features

SSIM 0.7087

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.features.webp" width="480" alt="mock hc setup.features"></td>
<td><img src="app-captures/hc/setup.features.webp" width="480" alt="app hc setup.features"></td>
</tr>
</table>

## hc / debit-note.default

SSIM 0.7095

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/debit-note.default.webp" width="480" alt="mock hc debit-note.default"></td>
<td><img src="app-captures/hc/debit-note.default.webp" width="480" alt="app hc debit-note.default"></td>
</tr>
</table>

## hc / contra.default

SSIM 0.7098

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/contra.default.webp" width="480" alt="mock hc contra.default"></td>
<td><img src="app-captures/hc/contra.default.webp" width="480" alt="app hc contra.default"></td>
</tr>
</table>

## hc / journal.default

SSIM 0.7104

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/journal.default.webp" width="480" alt="mock hc journal.default"></td>
<td><img src="app-captures/hc/journal.default.webp" width="480" alt="app hc journal.default"></td>
</tr>
</table>

## hc / payment.default

SSIM 0.7107

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/payment.default.webp" width="480" alt="mock hc payment.default"></td>
<td><img src="app-captures/hc/payment.default.webp" width="480" alt="app hc payment.default"></td>
</tr>
</table>

## hc / stock-summary.default

SSIM 0.7111

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/stock-summary.default.webp" width="480" alt="mock hc stock-summary.default"></td>
<td><img src="app-captures/hc/stock-summary.default.webp" width="480" alt="app hc stock-summary.default"></td>
</tr>
</table>

## dark / profit-and-loss.default

SSIM 0.7111

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.default.webp" width="480" alt="mock dark profit-and-loss.default"></td>
<td><img src="app-captures/dark/profit-and-loss.default.webp" width="480" alt="app dark profit-and-loss.default"></td>
</tr>
</table>

## light / payment.overdraft

SSIM 0.7138

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/payment.overdraft.webp" width="480" alt="mock light payment.overdraft"></td>
<td><img src="app-captures/light/payment.overdraft.webp" width="480" alt="app light payment.overdraft"></td>
</tr>
</table>

## dark / purchase.default

SSIM 0.7148

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/purchase.default.webp" width="480" alt="mock dark purchase.default"></td>
<td><img src="app-captures/dark/purchase.default.webp" width="480" alt="app dark purchase.default"></td>
</tr>
</table>

## dark / masters.items

SSIM 0.7172

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.items.webp" width="480" alt="mock dark masters.items"></td>
<td><img src="app-captures/dark/masters.items.webp" width="480" alt="app dark masters.items"></td>
</tr>
</table>

## light / bank.brs

SSIM 0.7175

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/bank.brs.webp" width="480" alt="mock light bank.brs"></td>
<td><img src="app-captures/light/bank.brs.webp" width="480" alt="app light bank.brs"></td>
</tr>
</table>

## light / masters.loading

SSIM 0.7179

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.loading.webp" width="480" alt="mock light masters.loading"></td>
<td><img src="app-captures/light/masters.loading.webp" width="480" alt="app light masters.loading"></td>
</tr>
</table>

## dark / payment.overdraft

SSIM 0.7186

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/payment.overdraft.webp" width="480" alt="mock dark payment.overdraft"></td>
<td><img src="app-captures/dark/payment.overdraft.webp" width="480" alt="app dark payment.overdraft"></td>
</tr>
</table>

## dark / bank.brs

SSIM 0.7190

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/bank.brs.webp" width="480" alt="mock dark bank.brs"></td>
<td><img src="app-captures/dark/bank.brs.webp" width="480" alt="app dark bank.brs"></td>
</tr>
</table>

## light / masters.items

SSIM 0.7195

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.items.webp" width="480" alt="mock light masters.items"></td>
<td><img src="app-captures/light/masters.items.webp" width="480" alt="app light masters.items"></td>
</tr>
</table>

## hc / setup.new-tab

SSIM 0.7204

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.new-tab.webp" width="480" alt="mock hc setup.new-tab"></td>
<td><img src="app-captures/hc/setup.new-tab.webp" width="480" alt="app hc setup.new-tab"></td>
</tr>
</table>

## hc / home.loading

SSIM 0.7220

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/home.loading.webp" width="480" alt="mock hc home.loading"></td>
<td><img src="app-captures/hc/home.loading.webp" width="480" alt="app hc home.loading"></td>
</tr>
</table>

## light / balance-sheet.voucher

SSIM 0.7230

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/balance-sheet.voucher.webp" width="480" alt="mock light balance-sheet.voucher"></td>
<td><img src="app-captures/light/balance-sheet.voucher.webp" width="480" alt="app light balance-sheet.voucher"></td>
</tr>
</table>

## dark / masters.loading

SSIM 0.7258

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.loading.webp" width="480" alt="mock dark masters.loading"></td>
<td><img src="app-captures/dark/masters.loading.webp" width="480" alt="app dark masters.loading"></td>
</tr>
</table>

## hc / audit.integrity

SSIM 0.7265

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/audit.integrity.webp" width="480" alt="mock hc audit.integrity"></td>
<td><img src="app-captures/hc/audit.integrity.webp" width="480" alt="app hc audit.integrity"></td>
</tr>
</table>

## light / receipt.default

SSIM 0.7267

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/receipt.default.webp" width="480" alt="mock light receipt.default"></td>
<td><img src="app-captures/light/receipt.default.webp" width="480" alt="app light receipt.default"></td>
</tr>
</table>

## light / stock-summary.default

SSIM 0.7276

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/stock-summary.default.webp" width="480" alt="mock light stock-summary.default"></td>
<td><img src="app-captures/light/stock-summary.default.webp" width="480" alt="app light stock-summary.default"></td>
</tr>
</table>

## light / balance-sheet.group

SSIM 0.7285

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/balance-sheet.group.webp" width="480" alt="mock light balance-sheet.group"></td>
<td><img src="app-captures/light/balance-sheet.group.webp" width="480" alt="app light balance-sheet.group"></td>
</tr>
</table>

## light / payment.bill-wise

SSIM 0.7289

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/payment.bill-wise.webp" width="480" alt="mock light payment.bill-wise"></td>
<td><img src="app-captures/light/payment.bill-wise.webp" width="480" alt="app light payment.bill-wise"></td>
</tr>
</table>

## light / credit-note.validation-error

SSIM 0.7320

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/credit-note.validation-error.webp" width="480" alt="mock light credit-note.validation-error"></td>
<td><img src="app-captures/light/credit-note.validation-error.webp" width="480" alt="app light credit-note.validation-error"></td>
</tr>
</table>

## light / gst.recon

SSIM 0.7321

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.recon.webp" width="480" alt="mock light gst.recon"></td>
<td><img src="app-captures/light/gst.recon.webp" width="480" alt="app light gst.recon"></td>
</tr>
</table>

## dark / balance-sheet.voucher

SSIM 0.7326

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/balance-sheet.voucher.webp" width="480" alt="mock dark balance-sheet.voucher"></td>
<td><img src="app-captures/dark/balance-sheet.voucher.webp" width="480" alt="app dark balance-sheet.voucher"></td>
</tr>
</table>

## hc / contra.validation-error

SSIM 0.7329

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/contra.validation-error.webp" width="480" alt="mock hc contra.validation-error"></td>
<td><img src="app-captures/hc/contra.validation-error.webp" width="480" alt="app hc contra.validation-error"></td>
</tr>
</table>

## light / debit-note.validation-error

SSIM 0.7341

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/debit-note.validation-error.webp" width="480" alt="mock light debit-note.validation-error"></td>
<td><img src="app-captures/light/debit-note.validation-error.webp" width="480" alt="app light debit-note.validation-error"></td>
</tr>
</table>

## hc / outstanding.payables

SSIM 0.7357

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/outstanding.payables.webp" width="480" alt="mock hc outstanding.payables"></td>
<td><img src="app-captures/hc/outstanding.payables.webp" width="480" alt="app hc outstanding.payables"></td>
</tr>
</table>

## dark / receipt.default

SSIM 0.7358

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/receipt.default.webp" width="480" alt="mock dark receipt.default"></td>
<td><img src="app-captures/dark/receipt.default.webp" width="480" alt="app dark receipt.default"></td>
</tr>
</table>

## dark / balance-sheet.ledger

SSIM 0.7363

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/balance-sheet.ledger.webp" width="480" alt="mock dark balance-sheet.ledger"></td>
<td><img src="app-captures/dark/balance-sheet.ledger.webp" width="480" alt="app dark balance-sheet.ledger"></td>
</tr>
</table>

## dark / credit-note.validation-error

SSIM 0.7366

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/credit-note.validation-error.webp" width="480" alt="mock dark credit-note.validation-error"></td>
<td><img src="app-captures/dark/credit-note.validation-error.webp" width="480" alt="app dark credit-note.validation-error"></td>
</tr>
</table>

## hc / day-book.default

SSIM 0.7368

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/day-book.default.webp" width="480" alt="mock hc day-book.default"></td>
<td><img src="app-captures/hc/day-book.default.webp" width="480" alt="app hc day-book.default"></td>
</tr>
</table>

## light / balance-sheet.ledger

SSIM 0.7371

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/balance-sheet.ledger.webp" width="480" alt="mock light balance-sheet.ledger"></td>
<td><img src="app-captures/light/balance-sheet.ledger.webp" width="480" alt="app light balance-sheet.ledger"></td>
</tr>
</table>

## hc / masters.ledger-detail

SSIM 0.7373

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.ledger-detail.webp" width="480" alt="mock hc masters.ledger-detail"></td>
<td><img src="app-captures/hc/masters.ledger-detail.webp" width="480" alt="app hc masters.ledger-detail"></td>
</tr>
</table>

## dark / balance-sheet.group

SSIM 0.7375

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/balance-sheet.group.webp" width="480" alt="mock dark balance-sheet.group"></td>
<td><img src="app-captures/dark/balance-sheet.group.webp" width="480" alt="app dark balance-sheet.group"></td>
</tr>
</table>

## light / outstanding.payables

SSIM 0.7378

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/outstanding.payables.webp" width="480" alt="mock light outstanding.payables"></td>
<td><img src="app-captures/light/outstanding.payables.webp" width="480" alt="app light outstanding.payables"></td>
</tr>
</table>

## dark / gst.recon

SSIM 0.7383

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.recon.webp" width="480" alt="mock dark gst.recon"></td>
<td><img src="app-captures/dark/gst.recon.webp" width="480" alt="app dark gst.recon"></td>
</tr>
</table>

## dark / payment.bill-wise

SSIM 0.7384

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/payment.bill-wise.webp" width="480" alt="mock dark payment.bill-wise"></td>
<td><img src="app-captures/dark/payment.bill-wise.webp" width="480" alt="app dark payment.bill-wise"></td>
</tr>
</table>

## dark / debit-note.validation-error

SSIM 0.7389

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/debit-note.validation-error.webp" width="480" alt="mock dark debit-note.validation-error"></td>
<td><img src="app-captures/dark/debit-note.validation-error.webp" width="480" alt="app dark debit-note.validation-error"></td>
</tr>
</table>

## dark / stock-summary.default

SSIM 0.7402

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/stock-summary.default.webp" width="480" alt="mock dark stock-summary.default"></td>
<td><img src="app-captures/dark/stock-summary.default.webp" width="480" alt="app dark stock-summary.default"></td>
</tr>
</table>

## light / purchase-bill.offline-ai

SSIM 0.7449

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/purchase-bill.offline-ai.webp" width="480" alt="mock light purchase-bill.offline-ai"></td>
<td><img src="app-captures/light/purchase-bill.offline-ai.webp" width="480" alt="app light purchase-bill.offline-ai"></td>
</tr>
</table>

## dark / outstanding.payables

SSIM 0.7461

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/outstanding.payables.webp" width="480" alt="mock dark outstanding.payables"></td>
<td><img src="app-captures/dark/outstanding.payables.webp" width="480" alt="app dark outstanding.payables"></td>
</tr>
</table>

## hc / bank.import-error

SSIM 0.7478

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/bank.import-error.webp" width="480" alt="mock hc bank.import-error"></td>
<td><img src="app-captures/hc/bank.import-error.webp" width="480" alt="app hc bank.import-error"></td>
</tr>
</table>

## hc / profit-and-loss.loading

SSIM 0.7488

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.loading.webp" width="480" alt="mock hc profit-and-loss.loading"></td>
<td><img src="app-captures/hc/profit-and-loss.loading.webp" width="480" alt="app hc profit-and-loss.loading"></td>
</tr>
</table>

## light / payment.default

SSIM 0.7498

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/payment.default.webp" width="480" alt="mock light payment.default"></td>
<td><img src="app-captures/light/payment.default.webp" width="480" alt="app light payment.default"></td>
</tr>
</table>

## hc / masters.create-ledger

SSIM 0.7521

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.create-ledger.webp" width="480" alt="mock hc masters.create-ledger"></td>
<td><img src="app-captures/hc/masters.create-ledger.webp" width="480" alt="app hc masters.create-ledger"></td>
</tr>
</table>

## dark / payment.default

SSIM 0.7562

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/payment.default.webp" width="480" alt="mock dark payment.default"></td>
<td><img src="app-captures/dark/payment.default.webp" width="480" alt="app dark payment.default"></td>
</tr>
</table>

## light / day-book.default

SSIM 0.7562

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/day-book.default.webp" width="480" alt="mock light day-book.default"></td>
<td><img src="app-captures/light/day-book.default.webp" width="480" alt="app light day-book.default"></td>
</tr>
</table>

## hc / home.empty

SSIM 0.7590

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/home.empty.webp" width="480" alt="mock hc home.empty"></td>
<td><img src="app-captures/hc/home.empty.webp" width="480" alt="app hc home.empty"></td>
</tr>
</table>

## light / setup.features

SSIM 0.7592

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.features.webp" width="480" alt="mock light setup.features"></td>
<td><img src="app-captures/light/setup.features.webp" width="480" alt="app light setup.features"></td>
</tr>
</table>

## hc / balance-sheet.loading

SSIM 0.7601

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/balance-sheet.loading.webp" width="480" alt="mock hc balance-sheet.loading"></td>
<td><img src="app-captures/hc/balance-sheet.loading.webp" width="480" alt="app hc balance-sheet.loading"></td>
</tr>
</table>

## light / credit-note.default

SSIM 0.7618

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/credit-note.default.webp" width="480" alt="mock light credit-note.default"></td>
<td><img src="app-captures/light/credit-note.default.webp" width="480" alt="app light credit-note.default"></td>
</tr>
</table>

## hc / masters.create-item

SSIM 0.7620

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.create-item.webp" width="480" alt="mock hc masters.create-item"></td>
<td><img src="app-captures/hc/masters.create-item.webp" width="480" alt="app hc masters.create-item"></td>
</tr>
</table>

## hc / masters.gstin-error

SSIM 0.7626

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.gstin-error.webp" width="480" alt="mock hc masters.gstin-error"></td>
<td><img src="app-captures/hc/masters.gstin-error.webp" width="480" alt="app hc masters.gstin-error"></td>
</tr>
</table>

## hc / ask.offline-ai

SSIM 0.7641

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ask.offline-ai.webp" width="480" alt="mock hc ask.offline-ai"></td>
<td><img src="app-captures/hc/ask.offline-ai.webp" width="480" alt="app hc ask.offline-ai"></td>
</tr>
</table>

## light / journal.validation-error

SSIM 0.7642

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/journal.validation-error.webp" width="480" alt="mock light journal.validation-error"></td>
<td><img src="app-captures/light/journal.validation-error.webp" width="480" alt="app light journal.validation-error"></td>
</tr>
</table>

## light / contra.default

SSIM 0.7646

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/contra.default.webp" width="480" alt="mock light contra.default"></td>
<td><img src="app-captures/light/contra.default.webp" width="480" alt="app light contra.default"></td>
</tr>
</table>

## light / setup.new-tab

SSIM 0.7648

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.new-tab.webp" width="480" alt="mock light setup.new-tab"></td>
<td><img src="app-captures/light/setup.new-tab.webp" width="480" alt="app light setup.new-tab"></td>
</tr>
</table>

## dark / day-book.default

SSIM 0.7656

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/day-book.default.webp" width="480" alt="mock dark day-book.default"></td>
<td><img src="app-captures/dark/day-book.default.webp" width="480" alt="app dark day-book.default"></td>
</tr>
</table>

## hc / stock-summary.loading

SSIM 0.7665

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/stock-summary.loading.webp" width="480" alt="mock hc stock-summary.loading"></td>
<td><img src="app-captures/hc/stock-summary.loading.webp" width="480" alt="app hc stock-summary.loading"></td>
</tr>
</table>

## light / debit-note.default

SSIM 0.7670

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/debit-note.default.webp" width="480" alt="mock light debit-note.default"></td>
<td><img src="app-captures/light/debit-note.default.webp" width="480" alt="app light debit-note.default"></td>
</tr>
</table>

## dark / credit-note.default

SSIM 0.7671

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/credit-note.default.webp" width="480" alt="mock dark credit-note.default"></td>
<td><img src="app-captures/dark/credit-note.default.webp" width="480" alt="app dark credit-note.default"></td>
</tr>
</table>

## hc / audit.loading

SSIM 0.7672

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/audit.loading.webp" width="480" alt="mock hc audit.loading"></td>
<td><img src="app-captures/hc/audit.loading.webp" width="480" alt="app hc audit.loading"></td>
</tr>
</table>

## hc / outstanding.loading

SSIM 0.7674

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/outstanding.loading.webp" width="480" alt="mock hc outstanding.loading"></td>
<td><img src="app-captures/hc/outstanding.loading.webp" width="480" alt="app hc outstanding.loading"></td>
</tr>
</table>

## dark / journal.validation-error

SSIM 0.7678

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/journal.validation-error.webp" width="480" alt="mock dark journal.validation-error"></td>
<td><img src="app-captures/dark/journal.validation-error.webp" width="480" alt="app dark journal.validation-error"></td>
</tr>
</table>

## hc / trial-balance.loading

SSIM 0.7680

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/trial-balance.loading.webp" width="480" alt="mock hc trial-balance.loading"></td>
<td><img src="app-captures/hc/trial-balance.loading.webp" width="480" alt="app hc trial-balance.loading"></td>
</tr>
</table>

## dark / setup.features

SSIM 0.7682

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.features.webp" width="480" alt="mock dark setup.features"></td>
<td><img src="app-captures/dark/setup.features.webp" width="480" alt="app dark setup.features"></td>
</tr>
</table>

## dark / setup.new-tab

SSIM 0.7700

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.new-tab.webp" width="480" alt="mock dark setup.new-tab"></td>
<td><img src="app-captures/dark/setup.new-tab.webp" width="480" alt="app dark setup.new-tab"></td>
</tr>
</table>

## dark / contra.default

SSIM 0.7703

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/contra.default.webp" width="480" alt="mock dark contra.default"></td>
<td><img src="app-captures/dark/contra.default.webp" width="480" alt="app dark contra.default"></td>
</tr>
</table>

## dark / debit-note.default

SSIM 0.7725

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/debit-note.default.webp" width="480" alt="mock dark debit-note.default"></td>
<td><img src="app-captures/dark/debit-note.default.webp" width="480" alt="app dark debit-note.default"></td>
</tr>
</table>

## hc / setup.company

SSIM 0.7763

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.company.webp" width="480" alt="mock hc setup.company"></td>
<td><img src="app-captures/hc/setup.company.webp" width="480" alt="app hc setup.company"></td>
</tr>
</table>

## dark / masters.ledger-detail

SSIM 0.7769

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.ledger-detail.webp" width="480" alt="mock dark masters.ledger-detail"></td>
<td><img src="app-captures/dark/masters.ledger-detail.webp" width="480" alt="app dark masters.ledger-detail"></td>
</tr>
</table>

## light / journal.default

SSIM 0.7782

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/journal.default.webp" width="480" alt="mock light journal.default"></td>
<td><img src="app-captures/light/journal.default.webp" width="480" alt="app light journal.default"></td>
</tr>
</table>

## light / masters.ledger-detail

SSIM 0.7802

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.ledger-detail.webp" width="480" alt="mock light masters.ledger-detail"></td>
<td><img src="app-captures/light/masters.ledger-detail.webp" width="480" alt="app light masters.ledger-detail"></td>
</tr>
</table>

## light / contra.validation-error

SSIM 0.7815

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/contra.validation-error.webp" width="480" alt="mock light contra.validation-error"></td>
<td><img src="app-captures/light/contra.validation-error.webp" width="480" alt="app light contra.validation-error"></td>
</tr>
</table>

## dark / journal.default

SSIM 0.7824

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/journal.default.webp" width="480" alt="mock dark journal.default"></td>
<td><img src="app-captures/dark/journal.default.webp" width="480" alt="app dark journal.default"></td>
</tr>
</table>

## hc / bank.empty

SSIM 0.7837

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/bank.empty.webp" width="480" alt="mock hc bank.empty"></td>
<td><img src="app-captures/hc/bank.empty.webp" width="480" alt="app hc bank.empty"></td>
</tr>
</table>

## dark / contra.validation-error

SSIM 0.7882

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/contra.validation-error.webp" width="480" alt="mock dark contra.validation-error"></td>
<td><img src="app-captures/dark/contra.validation-error.webp" width="480" alt="app dark contra.validation-error"></td>
</tr>
</table>

## hc / gst.recon-locked

SSIM 0.7901

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.recon-locked.webp" width="480" alt="mock hc gst.recon-locked"></td>
<td><img src="app-captures/hc/gst.recon-locked.webp" width="480" alt="app hc gst.recon-locked"></td>
</tr>
</table>

## hc / masters.items-empty

SSIM 0.7925

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.items-empty.webp" width="480" alt="mock hc masters.items-empty"></td>
<td><img src="app-captures/hc/masters.items-empty.webp" width="480" alt="app hc masters.items-empty"></td>
</tr>
</table>

## hc / masters.empty

SSIM 0.7935

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.empty.webp" width="480" alt="mock hc masters.empty"></td>
<td><img src="app-captures/hc/masters.empty.webp" width="480" alt="app hc masters.empty"></td>
</tr>
</table>

## dark / setup.company

SSIM 0.7954

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.company.webp" width="480" alt="mock dark setup.company"></td>
<td><img src="app-captures/dark/setup.company.webp" width="480" alt="app dark setup.company"></td>
</tr>
</table>

## hc / audit.unsupported

SSIM 0.7961

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/audit.unsupported.webp" width="480" alt="mock hc audit.unsupported"></td>
<td><img src="app-captures/hc/audit.unsupported.webp" width="480" alt="app hc audit.unsupported"></td>
</tr>
</table>

## light / masters.create-ledger

SSIM 0.7965

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.create-ledger.webp" width="480" alt="mock light masters.create-ledger"></td>
<td><img src="app-captures/light/masters.create-ledger.webp" width="480" alt="app light masters.create-ledger"></td>
</tr>
</table>

## light / setup.company

SSIM 0.7970

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.company.webp" width="480" alt="mock light setup.company"></td>
<td><img src="app-captures/light/setup.company.webp" width="480" alt="app light setup.company"></td>
</tr>
</table>

## dark / masters.create-ledger

SSIM 0.7974

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.create-ledger.webp" width="480" alt="mock dark masters.create-ledger"></td>
<td><img src="app-captures/dark/masters.create-ledger.webp" width="480" alt="app dark masters.create-ledger"></td>
</tr>
</table>

## light / masters.gstin-error

SSIM 0.7984

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.gstin-error.webp" width="480" alt="mock light masters.gstin-error"></td>
<td><img src="app-captures/light/masters.gstin-error.webp" width="480" alt="app light masters.gstin-error"></td>
</tr>
</table>

## light / audit.integrity

SSIM 0.7985

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/audit.integrity.webp" width="480" alt="mock light audit.integrity"></td>
<td><img src="app-captures/light/audit.integrity.webp" width="480" alt="app light audit.integrity"></td>
</tr>
</table>

## dark / bank.import-error

SSIM 0.7989

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/bank.import-error.webp" width="480" alt="mock dark bank.import-error"></td>
<td><img src="app-captures/dark/bank.import-error.webp" width="480" alt="app dark bank.import-error"></td>
</tr>
</table>

## dark / masters.gstin-error

SSIM 0.7989

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.gstin-error.webp" width="480" alt="mock dark masters.gstin-error"></td>
<td><img src="app-captures/dark/masters.gstin-error.webp" width="480" alt="app dark masters.gstin-error"></td>
</tr>
</table>

## hc / setup.gstin-error

SSIM 0.7989

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.gstin-error.webp" width="480" alt="mock hc setup.gstin-error"></td>
<td><img src="app-captures/hc/setup.gstin-error.webp" width="480" alt="app hc setup.gstin-error"></td>
</tr>
</table>

## dark / home.empty

SSIM 0.8001

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/home.empty.webp" width="480" alt="mock dark home.empty"></td>
<td><img src="app-captures/dark/home.empty.webp" width="480" alt="app dark home.empty"></td>
</tr>
</table>

## dark / audit.integrity

SSIM 0.8017

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/audit.integrity.webp" width="480" alt="mock dark audit.integrity"></td>
<td><img src="app-captures/dark/audit.integrity.webp" width="480" alt="app dark audit.integrity"></td>
</tr>
</table>

## light / home.loading

SSIM 0.8022

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/home.loading.webp" width="480" alt="mock light home.loading"></td>
<td><img src="app-captures/light/home.loading.webp" width="480" alt="app light home.loading"></td>
</tr>
</table>

## light / audit.loading

SSIM 0.8025

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/audit.loading.webp" width="480" alt="mock light audit.loading"></td>
<td><img src="app-captures/light/audit.loading.webp" width="480" alt="app light audit.loading"></td>
</tr>
</table>

## hc / gst.recon-empty

SSIM 0.8038

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.recon-empty.webp" width="480" alt="mock hc gst.recon-empty"></td>
<td><img src="app-captures/hc/gst.recon-empty.webp" width="480" alt="app hc gst.recon-empty"></td>
</tr>
</table>

## light / profit-and-loss.loading

SSIM 0.8044

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.loading.webp" width="480" alt="mock light profit-and-loss.loading"></td>
<td><img src="app-captures/light/profit-and-loss.loading.webp" width="480" alt="app light profit-and-loss.loading"></td>
</tr>
</table>

## light / home.empty

SSIM 0.8046

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/home.empty.webp" width="480" alt="mock light home.empty"></td>
<td><img src="app-captures/light/home.empty.webp" width="480" alt="app light home.empty"></td>
</tr>
</table>

## hc / setup.backup

SSIM 0.8052

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.backup.webp" width="480" alt="mock hc setup.backup"></td>
<td><img src="app-captures/hc/setup.backup.webp" width="480" alt="app hc setup.backup"></td>
</tr>
</table>

## hc / setup.ai

SSIM 0.8052

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.ai.webp" width="480" alt="mock hc setup.ai"></td>
<td><img src="app-captures/hc/setup.ai.webp" width="480" alt="app hc setup.ai"></td>
</tr>
</table>

## light / bank.import-error

SSIM 0.8052

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/bank.import-error.webp" width="480" alt="mock light bank.import-error"></td>
<td><img src="app-captures/light/bank.import-error.webp" width="480" alt="app light bank.import-error"></td>
</tr>
</table>

## hc / day-book.empty

SSIM 0.8067

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/day-book.empty.webp" width="480" alt="mock hc day-book.empty"></td>
<td><img src="app-captures/hc/day-book.empty.webp" width="480" alt="app hc day-book.empty"></td>
</tr>
</table>

## dark / masters.create-item

SSIM 0.8070

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.create-item.webp" width="480" alt="mock dark masters.create-item"></td>
<td><img src="app-captures/dark/masters.create-item.webp" width="480" alt="app dark masters.create-item"></td>
</tr>
</table>

## light / masters.create-item

SSIM 0.8075

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.create-item.webp" width="480" alt="mock light masters.create-item"></td>
<td><img src="app-captures/light/masters.create-item.webp" width="480" alt="app light masters.create-item"></td>
</tr>
</table>

## light / ask.offline-ai

SSIM 0.8077

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ask.offline-ai.webp" width="480" alt="mock light ask.offline-ai"></td>
<td><img src="app-captures/light/ask.offline-ai.webp" width="480" alt="app light ask.offline-ai"></td>
</tr>
</table>

## dark / bank.empty

SSIM 0.8108

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/bank.empty.webp" width="480" alt="mock dark bank.empty"></td>
<td><img src="app-captures/dark/bank.empty.webp" width="480" alt="app dark bank.empty"></td>
</tr>
</table>

## light / bank.empty

SSIM 0.8112

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/bank.empty.webp" width="480" alt="mock light bank.empty"></td>
<td><img src="app-captures/light/bank.empty.webp" width="480" alt="app light bank.empty"></td>
</tr>
</table>

## hc / bank-chooser.default

SSIM 0.8112

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/bank-chooser.default.webp" width="480" alt="mock hc bank-chooser.default"></td>
<td><img src="app-captures/hc/bank-chooser.default.webp" width="480" alt="app hc bank-chooser.default"></td>
</tr>
</table>

## hc / review.empty

SSIM 0.8123

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/review.empty.webp" width="480" alt="mock hc review.empty"></td>
<td><img src="app-captures/hc/review.empty.webp" width="480" alt="app hc review.empty"></td>
</tr>
</table>

## dark / gst.recon-locked

SSIM 0.8128

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.recon-locked.webp" width="480" alt="mock dark gst.recon-locked"></td>
<td><img src="app-captures/dark/gst.recon-locked.webp" width="480" alt="app dark gst.recon-locked"></td>
</tr>
</table>

## dark / ask.offline-ai

SSIM 0.8133

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ask.offline-ai.webp" width="480" alt="mock dark ask.offline-ai"></td>
<td><img src="app-captures/dark/ask.offline-ai.webp" width="480" alt="app dark ask.offline-ai"></td>
</tr>
</table>

## light / gst.recon-locked

SSIM 0.8135

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.recon-locked.webp" width="480" alt="mock light gst.recon-locked"></td>
<td><img src="app-captures/light/gst.recon-locked.webp" width="480" alt="app light gst.recon-locked"></td>
</tr>
</table>

## dark / setup.gstin-error

SSIM 0.8144

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.gstin-error.webp" width="480" alt="mock dark setup.gstin-error"></td>
<td><img src="app-captures/dark/setup.gstin-error.webp" width="480" alt="app dark setup.gstin-error"></td>
</tr>
</table>

## hc / setup.welcome

SSIM 0.8147

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.welcome.webp" width="480" alt="mock hc setup.welcome"></td>
<td><img src="app-captures/hc/setup.welcome.webp" width="480" alt="app hc setup.welcome"></td>
</tr>
</table>

## light / setup.gstin-error

SSIM 0.8154

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.gstin-error.webp" width="480" alt="mock light setup.gstin-error"></td>
<td><img src="app-captures/light/setup.gstin-error.webp" width="480" alt="app light setup.gstin-error"></td>
</tr>
</table>

## light / balance-sheet.loading

SSIM 0.8160

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/balance-sheet.loading.webp" width="480" alt="mock light balance-sheet.loading"></td>
<td><img src="app-captures/light/balance-sheet.loading.webp" width="480" alt="app light balance-sheet.loading"></td>
</tr>
</table>

## hc / gst.recon-error

SSIM 0.8173

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.recon-error.webp" width="480" alt="mock hc gst.recon-error"></td>
<td><img src="app-captures/hc/gst.recon-error.webp" width="480" alt="app hc gst.recon-error"></td>
</tr>
</table>

## light / stock-summary.loading

SSIM 0.8203

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/stock-summary.loading.webp" width="480" alt="mock light stock-summary.loading"></td>
<td><img src="app-captures/light/stock-summary.loading.webp" width="480" alt="app light stock-summary.loading"></td>
</tr>
</table>

## hc / setup.keyboard

SSIM 0.8206

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.keyboard.webp" width="480" alt="mock hc setup.keyboard"></td>
<td><img src="app-captures/hc/setup.keyboard.webp" width="480" alt="app hc setup.keyboard"></td>
</tr>
</table>

## dark / profit-and-loss.loading

SSIM 0.8206

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.loading.webp" width="480" alt="mock dark profit-and-loss.loading"></td>
<td><img src="app-captures/dark/profit-and-loss.loading.webp" width="480" alt="app dark profit-and-loss.loading"></td>
</tr>
</table>

## light / outstanding.loading

SSIM 0.8213

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/outstanding.loading.webp" width="480" alt="mock light outstanding.loading"></td>
<td><img src="app-captures/light/outstanding.loading.webp" width="480" alt="app light outstanding.loading"></td>
</tr>
</table>

## light / trial-balance.loading

SSIM 0.8220

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/trial-balance.loading.webp" width="480" alt="mock light trial-balance.loading"></td>
<td><img src="app-captures/light/trial-balance.loading.webp" width="480" alt="app light trial-balance.loading"></td>
</tr>
</table>

## light / audit.unsupported

SSIM 0.8249

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/audit.unsupported.webp" width="480" alt="mock light audit.unsupported"></td>
<td><img src="app-captures/light/audit.unsupported.webp" width="480" alt="app light audit.unsupported"></td>
</tr>
</table>

## dark / audit.unsupported

SSIM 0.8251

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/audit.unsupported.webp" width="480" alt="mock dark audit.unsupported"></td>
<td><img src="app-captures/dark/audit.unsupported.webp" width="480" alt="app dark audit.unsupported"></td>
</tr>
</table>

## dark / home.loading

SSIM 0.8256

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/home.loading.webp" width="480" alt="mock dark home.loading"></td>
<td><img src="app-captures/dark/home.loading.webp" width="480" alt="app dark home.loading"></td>
</tr>
</table>

## dark / masters.items-empty

SSIM 0.8272

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.items-empty.webp" width="480" alt="mock dark masters.items-empty"></td>
<td><img src="app-captures/dark/masters.items-empty.webp" width="480" alt="app dark masters.items-empty"></td>
</tr>
</table>

## dark / masters.empty

SSIM 0.8286

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.empty.webp" width="480" alt="mock dark masters.empty"></td>
<td><img src="app-captures/dark/masters.empty.webp" width="480" alt="app dark masters.empty"></td>
</tr>
</table>

## light / gst.recon-empty

SSIM 0.8296

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.recon-empty.webp" width="480" alt="mock light gst.recon-empty"></td>
<td><img src="app-captures/light/gst.recon-empty.webp" width="480" alt="app light gst.recon-empty"></td>
</tr>
</table>

## dark / gst.recon-empty

SSIM 0.8302

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.recon-empty.webp" width="480" alt="mock dark gst.recon-empty"></td>
<td><img src="app-captures/dark/gst.recon-empty.webp" width="480" alt="app dark gst.recon-empty"></td>
</tr>
</table>

## light / masters.items-empty

SSIM 0.8309

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.items-empty.webp" width="480" alt="mock light masters.items-empty"></td>
<td><img src="app-captures/light/masters.items-empty.webp" width="480" alt="app light masters.items-empty"></td>
</tr>
</table>

## dark / balance-sheet.loading

SSIM 0.8311

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/balance-sheet.loading.webp" width="480" alt="mock dark balance-sheet.loading"></td>
<td><img src="app-captures/dark/balance-sheet.loading.webp" width="480" alt="app dark balance-sheet.loading"></td>
</tr>
</table>

## dark / review.empty

SSIM 0.8316

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/review.empty.webp" width="480" alt="mock dark review.empty"></td>
<td><img src="app-captures/dark/review.empty.webp" width="480" alt="app dark review.empty"></td>
</tr>
</table>

## light / masters.empty

SSIM 0.8319

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.empty.webp" width="480" alt="mock light masters.empty"></td>
<td><img src="app-captures/light/masters.empty.webp" width="480" alt="app light masters.empty"></td>
</tr>
</table>

## light / review.empty

SSIM 0.8335

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/review.empty.webp" width="480" alt="mock light review.empty"></td>
<td><img src="app-captures/light/review.empty.webp" width="480" alt="app light review.empty"></td>
</tr>
</table>

## dark / audit.loading

SSIM 0.8338

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/audit.loading.webp" width="480" alt="mock dark audit.loading"></td>
<td><img src="app-captures/dark/audit.loading.webp" width="480" alt="app dark audit.loading"></td>
</tr>
</table>

## light / setup.backup

SSIM 0.8350

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.backup.webp" width="480" alt="mock light setup.backup"></td>
<td><img src="app-captures/light/setup.backup.webp" width="480" alt="app light setup.backup"></td>
</tr>
</table>

## light / day-book.empty

SSIM 0.8360

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/day-book.empty.webp" width="480" alt="mock light day-book.empty"></td>
<td><img src="app-captures/light/day-book.empty.webp" width="480" alt="app light day-book.empty"></td>
</tr>
</table>

## dark / stock-summary.loading

SSIM 0.8363

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/stock-summary.loading.webp" width="480" alt="mock dark stock-summary.loading"></td>
<td><img src="app-captures/dark/stock-summary.loading.webp" width="480" alt="app dark stock-summary.loading"></td>
</tr>
</table>

## dark / day-book.empty

SSIM 0.8367

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/day-book.empty.webp" width="480" alt="mock dark day-book.empty"></td>
<td><img src="app-captures/dark/day-book.empty.webp" width="480" alt="app dark day-book.empty"></td>
</tr>
</table>

## dark / outstanding.loading

SSIM 0.8372

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/outstanding.loading.webp" width="480" alt="mock dark outstanding.loading"></td>
<td><img src="app-captures/dark/outstanding.loading.webp" width="480" alt="app dark outstanding.loading"></td>
</tr>
</table>

## dark / setup.backup

SSIM 0.8374

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.backup.webp" width="480" alt="mock dark setup.backup"></td>
<td><img src="app-captures/dark/setup.backup.webp" width="480" alt="app dark setup.backup"></td>
</tr>
</table>

## dark / trial-balance.loading

SSIM 0.8379

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/trial-balance.loading.webp" width="480" alt="mock dark trial-balance.loading"></td>
<td><img src="app-captures/dark/trial-balance.loading.webp" width="480" alt="app dark trial-balance.loading"></td>
</tr>
</table>

## light / setup.welcome

SSIM 0.8417

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.welcome.webp" width="480" alt="mock light setup.welcome"></td>
<td><img src="app-captures/light/setup.welcome.webp" width="480" alt="app light setup.welcome"></td>
</tr>
</table>

## hc / gst.gstr3b-unsupported

SSIM 0.8426

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.gstr3b-unsupported.webp" width="480" alt="mock hc gst.gstr3b-unsupported"></td>
<td><img src="app-captures/hc/gst.gstr3b-unsupported.webp" width="480" alt="app hc gst.gstr3b-unsupported"></td>
</tr>
</table>

## dark / gst.recon-error

SSIM 0.8429

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.recon-error.webp" width="480" alt="mock dark gst.recon-error"></td>
<td><img src="app-captures/dark/gst.recon-error.webp" width="480" alt="app dark gst.recon-error"></td>
</tr>
</table>

## light / setup.keyboard

SSIM 0.8440

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.keyboard.webp" width="480" alt="mock light setup.keyboard"></td>
<td><img src="app-captures/light/setup.keyboard.webp" width="480" alt="app light setup.keyboard"></td>
</tr>
</table>

## dark / setup.keyboard

SSIM 0.8444

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.keyboard.webp" width="480" alt="mock dark setup.keyboard"></td>
<td><img src="app-captures/dark/setup.keyboard.webp" width="480" alt="app dark setup.keyboard"></td>
</tr>
</table>

## hc / start.recent

SSIM 0.8450

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/start.recent.webp" width="480" alt="mock hc start.recent"></td>
<td><img src="app-captures/hc/start.recent.webp" width="480" alt="app hc start.recent"></td>
</tr>
</table>

## hc / setup.rail

SSIM 0.8450

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.rail.webp" width="480" alt="mock hc setup.rail"></td>
<td><img src="app-captures/hc/setup.rail.webp" width="480" alt="app hc setup.rail"></td>
</tr>
</table>

## light / bank-chooser.default

SSIM 0.8459

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/bank-chooser.default.webp" width="480" alt="mock light bank-chooser.default"></td>
<td><img src="app-captures/light/bank-chooser.default.webp" width="480" alt="app light bank-chooser.default"></td>
</tr>
</table>

## dark / bank-chooser.default

SSIM 0.8470

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/bank-chooser.default.webp" width="480" alt="mock dark bank-chooser.default"></td>
<td><img src="app-captures/dark/bank-chooser.default.webp" width="480" alt="app dark bank-chooser.default"></td>
</tr>
</table>

## dark / setup.welcome

SSIM 0.8473

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.welcome.webp" width="480" alt="mock dark setup.welcome"></td>
<td><img src="app-captures/dark/setup.welcome.webp" width="480" alt="app dark setup.welcome"></td>
</tr>
</table>

## light / gst.recon-error

SSIM 0.8477

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.recon-error.webp" width="480" alt="mock light gst.recon-error"></td>
<td><img src="app-captures/light/gst.recon-error.webp" width="480" alt="app light gst.recon-error"></td>
</tr>
</table>

## light / setup.ai

SSIM 0.8483

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.ai.webp" width="480" alt="mock light setup.ai"></td>
<td><img src="app-captures/light/setup.ai.webp" width="480" alt="app light setup.ai"></td>
</tr>
</table>

## hc / gst.gstr1-not-loaded

SSIM 0.8492

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.gstr1-not-loaded.webp" width="480" alt="mock hc gst.gstr1-not-loaded"></td>
<td><img src="app-captures/hc/gst.gstr1-not-loaded.webp" width="480" alt="app hc gst.gstr1-not-loaded"></td>
</tr>
</table>

## hc / gst.gstr3b-not-loaded

SSIM 0.8511

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.gstr3b-not-loaded.webp" width="480" alt="mock hc gst.gstr3b-not-loaded"></td>
<td><img src="app-captures/hc/gst.gstr3b-not-loaded.webp" width="480" alt="app hc gst.gstr3b-not-loaded"></td>
</tr>
</table>

## dark / setup.ai

SSIM 0.8513

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.ai.webp" width="480" alt="mock dark setup.ai"></td>
<td><img src="app-captures/dark/setup.ai.webp" width="480" alt="app dark setup.ai"></td>
</tr>
</table>

## dark / gst.gstr3b-unsupported

SSIM 0.8559

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.gstr3b-unsupported.webp" width="480" alt="mock dark gst.gstr3b-unsupported"></td>
<td><img src="app-captures/dark/gst.gstr3b-unsupported.webp" width="480" alt="app dark gst.gstr3b-unsupported"></td>
</tr>
</table>

## light / gst.gstr3b-unsupported

SSIM 0.8593

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.gstr3b-unsupported.webp" width="480" alt="mock light gst.gstr3b-unsupported"></td>
<td><img src="app-captures/light/gst.gstr3b-unsupported.webp" width="480" alt="app light gst.gstr3b-unsupported"></td>
</tr>
</table>

## light / gst.gstr1-not-loaded

SSIM 0.8617

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.gstr1-not-loaded.webp" width="480" alt="mock light gst.gstr1-not-loaded"></td>
<td><img src="app-captures/light/gst.gstr1-not-loaded.webp" width="480" alt="app light gst.gstr1-not-loaded"></td>
</tr>
</table>

## light / gst.gstr3b-not-loaded

SSIM 0.8622

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.gstr3b-not-loaded.webp" width="480" alt="mock light gst.gstr3b-not-loaded"></td>
<td><img src="app-captures/light/gst.gstr3b-not-loaded.webp" width="480" alt="app light gst.gstr3b-not-loaded"></td>
</tr>
</table>

## dark / gst.gstr1-not-loaded

SSIM 0.8631

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.gstr1-not-loaded.webp" width="480" alt="mock dark gst.gstr1-not-loaded"></td>
<td><img src="app-captures/dark/gst.gstr1-not-loaded.webp" width="480" alt="app dark gst.gstr1-not-loaded"></td>
</tr>
</table>

## dark / gst.gstr3b-not-loaded

SSIM 0.8637

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.gstr3b-not-loaded.webp" width="480" alt="mock dark gst.gstr3b-not-loaded"></td>
<td><img src="app-captures/dark/gst.gstr3b-not-loaded.webp" width="480" alt="app dark gst.gstr3b-not-loaded"></td>
</tr>
</table>

## light / setup.rail

SSIM 0.8661

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.rail.webp" width="480" alt="mock light setup.rail"></td>
<td><img src="app-captures/light/setup.rail.webp" width="480" alt="app light setup.rail"></td>
</tr>
</table>

## dark / start.recent

SSIM 0.8664

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/start.recent.webp" width="480" alt="mock dark start.recent"></td>
<td><img src="app-captures/dark/start.recent.webp" width="480" alt="app dark start.recent"></td>
</tr>
</table>

## hc / setup.done

SSIM 0.8667

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.done.webp" width="480" alt="mock hc setup.done"></td>
<td><img src="app-captures/hc/setup.done.webp" width="480" alt="app hc setup.done"></td>
</tr>
</table>

## light / start.recent

SSIM 0.8669

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/start.recent.webp" width="480" alt="mock light start.recent"></td>
<td><img src="app-captures/light/start.recent.webp" width="480" alt="app light start.recent"></td>
</tr>
</table>

## dark / setup.rail

SSIM 0.8719

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.rail.webp" width="480" alt="mock dark setup.rail"></td>
<td><img src="app-captures/dark/setup.rail.webp" width="480" alt="app dark setup.rail"></td>
</tr>
</table>

## light / setup.done

SSIM 0.8926

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.done.webp" width="480" alt="mock light setup.done"></td>
<td><img src="app-captures/light/setup.done.webp" width="480" alt="app light setup.done"></td>
</tr>
</table>

## dark / setup.done

SSIM 0.8950

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.done.webp" width="480" alt="mock dark setup.done"></td>
<td><img src="app-captures/dark/setup.done.webp" width="480" alt="app dark setup.done"></td>
</tr>
</table>

## Mock states with no app capture yet

### light (25)

- `ask.result`
- `ask.loading`
- `ask.low-confidence`
- `ask.unsupported`
- `purchase-bill.reading`
- `purchase-bill.proposal`
- `purchase-bill.low-confidence`
- `purchase-bill.mismatch`
- `purchase-bill.duplicate`
- `purchase-bill.cloud-consent`
- `purchase-bill.error`
- `purchase-bill.accepted`
- `quick-entry.proposal`
- `quick-entry.hinglish`
- `quick-entry.unknown-ledger`
- `quick-entry.loading`
- `quick-entry.rejected`
- `gst.recon-explain`
- `bank.ai-suggestions`
- `bank.suggested-ledger`
- `masters.suggested-ledger`
- `profit-and-loss.print-error`
- `gst.gstr1-failed`
- `audit.failed`
- `gst.gstr1-unsupported`

### dark (25)

- `ask.result`
- `ask.loading`
- `ask.low-confidence`
- `ask.unsupported`
- `purchase-bill.reading`
- `purchase-bill.proposal`
- `purchase-bill.low-confidence`
- `purchase-bill.mismatch`
- `purchase-bill.duplicate`
- `purchase-bill.cloud-consent`
- `purchase-bill.error`
- `purchase-bill.accepted`
- `quick-entry.proposal`
- `quick-entry.hinglish`
- `quick-entry.unknown-ledger`
- `quick-entry.loading`
- `quick-entry.rejected`
- `gst.recon-explain`
- `bank.ai-suggestions`
- `bank.suggested-ledger`
- `masters.suggested-ledger`
- `profit-and-loss.print-error`
- `gst.gstr1-failed`
- `audit.failed`
- `gst.gstr1-unsupported`

### hc (25)

- `ask.result`
- `ask.loading`
- `ask.low-confidence`
- `ask.unsupported`
- `purchase-bill.reading`
- `purchase-bill.proposal`
- `purchase-bill.low-confidence`
- `purchase-bill.mismatch`
- `purchase-bill.duplicate`
- `purchase-bill.cloud-consent`
- `purchase-bill.error`
- `purchase-bill.accepted`
- `quick-entry.proposal`
- `quick-entry.hinglish`
- `quick-entry.unknown-ledger`
- `quick-entry.loading`
- `quick-entry.rejected`
- `gst.recon-explain`
- `bank.ai-suggestions`
- `bank.suggested-ledger`
- `masters.suggested-ledger`
- `profit-and-loss.print-error`
- `gst.gstr1-failed`
- `audit.failed`
- `gst.gstr1-unsupported`

## App captures with no mock capture

- `light/quick-entry.offline-ai`

