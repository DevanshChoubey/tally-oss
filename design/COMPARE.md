# App against mock

525 app captures are paired with a mock capture of the same theme and state id. SSIM is 1 for identical images; lower is a bigger difference, and the list runs worst first.

| Theme | State | SSIM |
| --- | --- | --- |
| hc | import.file-error | 0.4249 |
| hc | import.file-transactions | 0.4363 |
| hc | import.file-overwritten | 0.4396 |
| hc | import.file-masters | 0.4512 |
| hc | import.file-imported | 0.4630 |
| hc | import.file-backed-up | 0.4660 |
| hc | hi.import.file-overwritten | 0.4675 |
| hc | hi.import.file-transactions | 0.4684 |
| hc | hi.import.file-masters | 0.4712 |
| hc | hi.import.file-imported | 0.4713 |
| hc | hi.import.file-error | 0.4733 |
| hc | import.file-missing-choice | 0.4766 |
| hc | sales-invoice.save-confirm | 0.4793 |
| hc | hi.import.file-backed-up | 0.4815 |
| hc | profit-and-loss.print-config | 0.4926 |
| hc | backup.restore | 0.5082 |
| hc | audit.default | 0.5163 |
| hc | day-book.month | 0.5180 |
| hc | profit-and-loss.print-failed | 0.5193 |
| dark | profit-and-loss.print-config | 0.5209 |
| hc | masters.loading | 0.5231 |
| hc | profit-and-loss.provisional | 0.5268 |
| hc | ai.ai-settings.local-downloading | 0.5306 |
| hc | sales-invoice.as-voucher | 0.5308 |
| hc | ai.ai-settings.local-ready | 0.5311 |
| hc | profit-and-loss.detailed | 0.5316 |
| hc | hi.import.file-missing-choice | 0.5331 |
| hc | home.nudge-open | 0.5350 |
| hc | home.gst-unsupported | 0.5479 |
| hc | import.port-error | 0.5501 |
| hc | import.validation-error | 0.5511 |
| hc | ai.purchase-bill.needs-review | 0.5525 |
| hc | bank.default | 0.5526 |
| hc | bank.offline-ai | 0.5526 |
| hc | ai.review.default | 0.5536 |
| hc | review.default | 0.5537 |
| hc | import.imported | 0.5544 |
| hc | stock-summary.provisional | 0.5556 |
| dark | import.file-transactions | 0.5559 |
| hc | sales-invoice.einvoice-not-ready | 0.5598 |
| dark | import.file-masters | 0.5599 |
| hc | import.no-company | 0.5614 |
| hc | hi.bank.default | 0.5637 |
| light | import.file-transactions | 0.5640 |
| hc | backup.backed-up | 0.5648 |
| hc | gst.gstr3b | 0.5649 |
| light | import.file-masters | 0.5656 |
| hc | profit-and-loss.print-range-error | 0.5657 |
| dark | profit-and-loss.print-range-error | 0.5667 |
| hc | import.failed | 0.5694 |
| hc | import.refused | 0.5702 |
| hc | ai.quick-entry.proposal | 0.5709 |
| dark | import.file-overwritten | 0.5713 |
| light | import.file-overwritten | 0.5717 |
| hc | trial-balance.default | 0.5751 |
| dark | hi.import.file-transactions | 0.5772 |
| light | import.file-error | 0.5779 |
| dark | profit-and-loss.print-preview | 0.5779 |
| hc | profit-and-loss.print-preview | 0.5780 |
| hc | ai-settings.off | 0.5786 |
| hc | ai.ai-settings.cloud | 0.5792 |
| dark | import.file-error | 0.5793 |
| hc | ai.quick-entry.rejected | 0.5795 |
| hc | masters.ledgers | 0.5818 |
| dark | import.file-imported | 0.5823 |
| hc | ai.purchase-bill.ledger-lines | 0.5848 |
| light | hi.import.file-transactions | 0.5862 |
| hc | ai.ai-settings.local-download-failed | 0.5874 |
| light | import.file-imported | 0.5883 |
| dark | hi.import.file-masters | 0.5910 |
| hc | import.default | 0.5916 |
| hc | sales-invoice.item-list | 0.5917 |
| hc | import.running | 0.5917 |
| hc | bank.brs-exported | 0.5922 |
| light | audit.default | 0.5922 |
| hc | purchase.validation-error | 0.5928 |
| hc | home.ai-off | 0.5928 |
| hc | home.default | 0.5928 |
| hc | hi.masters.ledgers | 0.5939 |
| dark | hi.import.file-backed-up | 0.5970 |
| dark | import.file-backed-up | 0.5971 |
| light | ai.purchase-bill.needs-review | 0.5981 |
| light | hi.import.file-masters | 0.5982 |
| light | import.file-backed-up | 0.5990 |
| hc | home.new-tab | 0.6005 |
| hc | ai.ask.result | 0.6008 |
| hc | sales-invoice.period-locked | 0.6012 |
| hc | keys.standard | 0.6014 |
| light | import.file-missing-choice | 0.6019 |
| hc | sales-invoice.validation-error | 0.6024 |
| dark | backup.restore | 0.6025 |
| hc | balance-sheet.provisional | 0.6031 |
| dark | ai.purchase-bill.needs-review | 0.6034 |
| light | hi.import.file-backed-up | 0.6037 |
| hc | ai.ai-settings.cloud-edit | 0.6040 |
| hc | ai.quick-entry.unknown-ledger | 0.6044 |
| light | ai.review.default | 0.6046 |
| light | backup.restore | 0.6064 |
| light | ai.ai-settings.local-downloading | 0.6067 |
| dark | hi.import.file-overwritten | 0.6075 |
| hc | sales-invoice.mode-list | 0.6075 |
| light | ai.ai-settings.local-ready | 0.6078 |
| light | review.default | 0.6078 |
| light | masters.ledgers | 0.6078 |
| light | sales-invoice.save-confirm | 0.6083 |
| hc | balance-sheet.detailed | 0.6084 |
| dark | audit.default | 0.6086 |
| dark | review.default | 0.6091 |
| hc | gst.gstr1-correction | 0.6094 |
| hc | ai.ai-settings.local | 0.6095 |
| hc | gst.gstr1 | 0.6095 |
| dark | ai.review.default | 0.6098 |
| dark | bank.default | 0.6106 |
| dark | bank.offline-ai | 0.6106 |
| light | hi.import.file-overwritten | 0.6110 |
| hc | keys.classic | 0.6117 |
| hc | ai.quick-entry.accepted | 0.6125 |
| light | gst.gstr3b | 0.6126 |
| hc | masters.items | 0.6133 |
| hc | profit-and-loss.print-busy | 0.6143 |
| hc | balance-sheet.default | 0.6149 |
| hc | profit-and-loss.print-saved | 0.6177 |
| dark | import.file-missing-choice | 0.6182 |
| light | balance-sheet.provisional | 0.6184 |
| light | hi.masters.ledgers | 0.6190 |
| dark | hi.import.file-imported | 0.6191 |
| hc | sales-invoice.gst-inter-state | 0.6199 |
| dark | hi.import.file-error | 0.6201 |
| hc | masters.no-match | 0.6203 |
| light | home.nudge-open | 0.6207 |
| dark | gst.gstr3b | 0.6208 |
| light | sales-invoice.einvoice-not-ready | 0.6209 |
| dark | ai.ai-settings.local-downloading | 0.6211 |
| dark | sales-invoice.save-confirm | 0.6220 |
| light | trial-balance.default | 0.6221 |
| dark | ai.ai-settings.local-ready | 0.6222 |
| light | hi.import.file-error | 0.6222 |
| light | day-book.month | 0.6222 |
| light | sales-invoice.period-locked | 0.6228 |
| light | ai.purchase-bill.ledger-lines | 0.6242 |
| hc | hi.gst.gstr1 | 0.6252 |
| hc | sales-invoice.default | 0.6257 |
| hc | hi.balance-sheet.default | 0.6258 |
| dark | hi.bank.default | 0.6282 |
| light | hi.import.file-imported | 0.6282 |
| dark | home.nudge-open | 0.6283 |
| light | profit-and-loss.detailed | 0.6295 |
| hc | bank.brs | 0.6307 |
| light | bank.default | 0.6309 |
| light | bank.offline-ai | 0.6309 |
| light | sales-invoice.validation-error | 0.6316 |
| dark | ai.purchase-bill.ledger-lines | 0.6316 |
| hc | ai.purchase-bill.reading | 0.6324 |
| hc | sales-invoice.print-preview | 0.6330 |
| dark | day-book.month | 0.6335 |
| dark | ai.quick-entry.proposal | 0.6335 |
| hc | hi.sales-invoice.default | 0.6336 |
| light | sales-invoice.default | 0.6336 |
| dark | home.gst-unsupported | 0.6340 |
| light | sales-invoice.as-voucher | 0.6345 |
| hc | backup.default | 0.6345 |
| light | sales-invoice.item-list | 0.6347 |
| light | hi.import.file-missing-choice | 0.6359 |
| light | ai.quick-entry.proposal | 0.6373 |
| dark | sales-invoice.print-preview | 0.6378 |
| hc | hi.home.default | 0.6378 |
| light | sales-invoice.gst-inter-state | 0.6379 |
| dark | masters.ledgers | 0.6385 |
| dark | sales-invoice.einvoice-not-ready | 0.6388 |
| hc | day-book.print-preview | 0.6395 |
| light | ai.quick-entry.rejected | 0.6396 |
| dark | ai.quick-entry.rejected | 0.6399 |
| dark | day-book.print-preview | 0.6408 |
| light | ai.ai-settings.local-download-failed | 0.6413 |
| light | home.gst-unsupported | 0.6420 |
| dark | sales-invoice.period-locked | 0.6421 |
| dark | sales-invoice.validation-error | 0.6423 |
| light | ai.ai-settings.local | 0.6424 |
| dark | sales-invoice.item-list | 0.6427 |
| hc | hi.sales-invoice.print-preview | 0.6428 |
| light | ai.ai-settings.cloud | 0.6431 |
| dark | backup.backed-up | 0.6433 |
| hc | credit-note.validation-error | 0.6435 |
| light | hi.sales-invoice.default | 0.6448 |
| dark | ai.ai-settings.cloud | 0.6464 |
| hc | debit-note.validation-error | 0.6469 |
| light | sales-invoice.mode-list | 0.6469 |
| dark | sales-invoice.as-voucher | 0.6474 |
| dark | hi.import.file-missing-choice | 0.6480 |
| light | profit-and-loss.provisional | 0.6486 |
| dark | hi.sales-invoice.print-preview | 0.6489 |
| dark | hi.masters.ledgers | 0.6497 |
| dark | sales-invoice.mode-list | 0.6497 |
| light | hi.bank.default | 0.6501 |
| dark | ai.ai-settings.local | 0.6516 |
| dark | bank.brs-exported | 0.6520 |
| dark | balance-sheet.provisional | 0.6525 |
| hc | sales-invoice.b2c | 0.6528 |
| dark | ai.ai-settings.local-download-failed | 0.6533 |
| dark | import.default | 0.6537 |
| dark | profit-and-loss.detailed | 0.6538 |
| dark | sales-invoice.default | 0.6539 |
| light | backup.backed-up | 0.6544 |
| light | bank.brs-exported | 0.6544 |
| light | ai.ask.result | 0.6559 |
| dark | trial-balance.default | 0.6561 |
| dark | ai.ask.result | 0.6563 |
| dark | ai.ai-settings.cloud-edit | 0.6567 |
| light | profit-and-loss.print-busy | 0.6569 |
| dark | profit-and-loss.provisional | 0.6570 |
| dark | home.new-tab | 0.6571 |
| light | keys.standard | 0.6575 |
| hc | palette.default | 0.6580 |
| dark | keys.standard | 0.6583 |
| hc | palette.voucher | 0.6586 |
| hc | purchase.default | 0.6588 |
| dark | sales-invoice.gst-inter-state | 0.6594 |
| light | home.new-tab | 0.6596 |
| dark | import.refused | 0.6596 |
| dark | import.port-error | 0.6602 |
| light | import.refused | 0.6603 |
| light | import.default | 0.6604 |
| dark | import.failed | 0.6605 |
| light | import.failed | 0.6606 |
| light | balance-sheet.detailed | 0.6610 |
| dark | import.validation-error | 0.6618 |
| light | profit-and-loss.print-failed | 0.6623 |
| hc | palette.no-results | 0.6631 |
| dark | profit-and-loss.print-failed | 0.6632 |
| light | import.port-error | 0.6636 |
| light | ai.ai-settings.cloud-edit | 0.6640 |
| dark | stock-summary.provisional | 0.6642 |
| dark | hi.sales-invoice.default | 0.6643 |
| light | stock-summary.provisional | 0.6644 |
| light | import.validation-error | 0.6650 |
| hc | sales-invoice.saved | 0.6662 |
| light | profit-and-loss.print-range-error | 0.6665 |
| hc | palette.search | 0.6665 |
| light | sales-invoice.b2c | 0.6666 |
| dark | home.ai-off | 0.6669 |
| dark | home.default | 0.6669 |
| hc | journal.print-preview | 0.6679 |
| hc | profit-and-loss.default | 0.6686 |
| light | balance-sheet.default | 0.6687 |
| light | keys.classic | 0.6690 |
| dark | keys.classic | 0.6692 |
| light | sales-invoice.print-preview | 0.6692 |
| hc | balance-sheet.voucher | 0.6710 |
| light | home.ai-off | 0.6712 |
| light | home.default | 0.6712 |
| dark | import.no-company | 0.6715 |
| hc | balance-sheet.ledger | 0.6716 |
| light | import.imported | 0.6717 |
| dark | profit-and-loss.print-busy | 0.6723 |
| dark | import.imported | 0.6725 |
| light | hi.sales-invoice.print-preview | 0.6737 |
| dark | ai.purchase-bill.reading | 0.6747 |
| dark | ai.quick-entry.unknown-ledger | 0.6750 |
| dark | import.running | 0.6753 |
| light | bank.brs | 0.6755 |
| light | ai.quick-entry.unknown-ledger | 0.6756 |
| light | profit-and-loss.print-config | 0.6756 |
| hc | payment.overdraft | 0.6758 |
| light | import.running | 0.6769 |
| light | profit-and-loss.print-preview | 0.6771 |
| light | hi.balance-sheet.default | 0.6787 |
| light | gst.gstr1 | 0.6792 |
| dark | bank.brs | 0.6797 |
| dark | gst.gstr1 | 0.6834 |
| light | import.no-company | 0.6834 |
| dark | journal.print-preview | 0.6843 |
| light | ai-settings.off | 0.6865 |
| light | sales-invoice.saved | 0.6867 |
| dark | sales-invoice.b2c | 0.6867 |
| hc | outstanding.receivables | 0.6886 |
| hc | journal.validation-error | 0.6891 |
| light | gst.gstr1-correction | 0.6901 |
| dark | ai-settings.off | 0.6906 |
| dark | backup.default | 0.6909 |
| dark | ai.quick-entry.accepted | 0.6928 |
| light | hi.gst.gstr1 | 0.6936 |
| dark | gst.gstr1-correction | 0.6946 |
| hc | receipt.default | 0.6956 |
| light | ai.quick-entry.accepted | 0.6959 |
| hc | palette.commands | 0.6972 |
| dark | masters.no-match | 0.6979 |
| light | hi.home.default | 0.6982 |
| hc | payment.bill-wise | 0.6985 |
| dark | hi.gst.gstr1 | 0.6987 |
| hc | credit-note.default | 0.6988 |
| light | outstanding.receivables | 0.6988 |
| dark | balance-sheet.detailed | 0.7009 |
| dark | sales-invoice.saved | 0.7016 |
| dark | hi.home.default | 0.7025 |
| light | purchase.validation-error | 0.7033 |
| light | masters.no-match | 0.7041 |
| light | backup.default | 0.7059 |
| dark | balance-sheet.default | 0.7063 |
| hc | debit-note.default | 0.7066 |
| light | profit-and-loss.default | 0.7073 |
| dark | purchase.validation-error | 0.7075 |
| hc | ai.ask.unsupported | 0.7101 |
| light | profit-and-loss.print-saved | 0.7102 |
| dark | outstanding.receivables | 0.7112 |
| light | purchase.default | 0.7112 |
| hc | setup.features | 0.7130 |
| hc | balance-sheet.group | 0.7135 |
| hc | gst.recon | 0.7147 |
| light | palette.default | 0.7156 |
| hc | contra.default | 0.7158 |
| dark | hi.balance-sheet.default | 0.7163 |
| hc | journal.default | 0.7165 |
| hc | stock-summary.default | 0.7171 |
| light | payment.overdraft | 0.7183 |
| dark | profit-and-loss.print-saved | 0.7202 |
| dark | purchase.default | 0.7202 |
| light | palette.search | 0.7203 |
| light | palette.voucher | 0.7213 |
| light | day-book.print-preview | 0.7221 |
| dark | masters.items | 0.7227 |
| hc | ai.quick-entry.loading | 0.7227 |
| dark | palette.default | 0.7232 |
| light | masters.loading | 0.7234 |
| hc | payment.default | 0.7240 |
| dark | profit-and-loss.default | 0.7241 |
| light | palette.no-results | 0.7247 |
| dark | payment.overdraft | 0.7247 |
| light | masters.items | 0.7250 |
| hc | ai.quick-entry.not-installed | 0.7251 |
| light | credit-note.validation-error | 0.7262 |
| light | debit-note.validation-error | 0.7275 |
| hc | setup.new-tab | 0.7278 |
| hc | home.loading | 0.7280 |
| light | balance-sheet.voucher | 0.7284 |
| hc | payment.account-list | 0.7297 |
| light | receipt.default | 0.7312 |
| dark | masters.loading | 0.7313 |
| light | stock-summary.default | 0.7329 |
| dark | credit-note.validation-error | 0.7330 |
| light | payment.bill-wise | 0.7335 |
| hc | ai.ask.failed | 0.7338 |
| light | balance-sheet.group | 0.7339 |
| dark | debit-note.validation-error | 0.7345 |
| dark | palette.search | 0.7347 |
| light | gst.recon | 0.7376 |
| dark | balance-sheet.voucher | 0.7380 |
| hc | masters.ledger-detail | 0.7380 |
| light | palette.commands | 0.7380 |
| dark | palette.no-results | 0.7385 |
| hc | contra.validation-error | 0.7389 |
| dark | palette.voucher | 0.7395 |
| dark | receipt.default | 0.7403 |
| dark | balance-sheet.ledger | 0.7416 |
| light | balance-sheet.ledger | 0.7425 |
| dark | balance-sheet.group | 0.7429 |
| dark | payment.bill-wise | 0.7430 |
| dark | gst.recon | 0.7437 |
| hc | day-book.default | 0.7441 |
| light | outstanding.payables | 0.7444 |
| hc | ai.quick-entry.read-failed | 0.7452 |
| dark | stock-summary.default | 0.7454 |
| hc | outstanding.payables | 0.7474 |
| dark | palette.commands | 0.7500 |
| dark | outstanding.payables | 0.7526 |
| light | payment.default | 0.7528 |
| hc | bank.import-error | 0.7539 |
| hc | profit-and-loss.loading | 0.7562 |
| hc | ai.ask.idle | 0.7571 |
| hc | masters.create-ledger | 0.7578 |
| light | credit-note.default | 0.7585 |
| hc | ai.purchase-bill.rejected | 0.7587 |
| dark | payment.default | 0.7602 |
| light | ai.quick-entry.loading | 0.7610 |
| light | day-book.default | 0.7621 |
| light | debit-note.default | 0.7626 |
| light | ai.ask.unsupported | 0.7636 |
| dark | credit-note.default | 0.7642 |
| light | setup.features | 0.7643 |
| hc | home.empty | 0.7647 |
| hc | audit.integrity | 0.7658 |
| light | payment.account-list | 0.7662 |
| hc | balance-sheet.loading | 0.7664 |
| hc | masters.gstin-error | 0.7676 |
| hc | ai.quick-entry.idle | 0.7678 |
| dark | ai.ask.unsupported | 0.7691 |
| dark | debit-note.default | 0.7692 |
| dark | ai.purchase-bill.rejected | 0.7694 |
| light | journal.validation-error | 0.7695 |
| light | ai.purchase-bill.rejected | 0.7696 |
| light | contra.default | 0.7699 |
| dark | masters.ledger-detail | 0.7703 |
| dark | ai.quick-entry.loading | 0.7712 |
| dark | day-book.default | 0.7717 |
| light | ai.purchase-bill.reading | 0.7718 |
| light | setup.new-tab | 0.7721 |
| dark | setup.features | 0.7724 |
| hc | stock-summary.loading | 0.7726 |
| light | ai.quick-entry.not-installed | 0.7726 |
| dark | journal.validation-error | 0.7729 |
| dark | ai.quick-entry.not-installed | 0.7731 |
| hc | audit.loading | 0.7733 |
| hc | outstanding.loading | 0.7734 |
| dark | payment.account-list | 0.7735 |
| hc | trial-balance.loading | 0.7741 |
| dark | contra.default | 0.7755 |
| dark | setup.new-tab | 0.7764 |
| light | masters.ledger-detail | 0.7781 |
| light | ai.ask.failed | 0.7787 |
| hc | setup.company | 0.7806 |
| light | journal.print-preview | 0.7810 |
| dark | ai.ask.failed | 0.7821 |
| light | journal.default | 0.7835 |
| dark | ai.quick-entry.read-failed | 0.7853 |
| light | contra.validation-error | 0.7868 |
| light | ai.quick-entry.read-failed | 0.7873 |
| dark | journal.default | 0.7876 |
| light | ai.ask.idle | 0.7880 |
| hc | bank.empty | 0.7898 |
| dark | ai.ask.idle | 0.7899 |
| hc | masters.create-item | 0.7902 |
| dark | contra.validation-error | 0.7934 |
| hc | gst.recon-locked | 0.7962 |
| hc | masters.items-empty | 0.7986 |
| dark | setup.company | 0.7995 |
| hc | masters.empty | 0.7996 |
| light | masters.gstin-error | 0.8010 |
| light | ai.quick-entry.idle | 0.8015 |
| dark | masters.gstin-error | 0.8018 |
| light | masters.create-ledger | 0.8020 |
| light | setup.company | 0.8021 |
| dark | ai.quick-entry.idle | 0.8027 |
| dark | masters.create-ledger | 0.8028 |
| hc | setup.gstin-error | 0.8039 |
| dark | bank.import-error | 0.8040 |
| hc | audit.unsupported | 0.8040 |
| dark | home.empty | 0.8045 |
| light | home.loading | 0.8074 |
| light | audit.loading | 0.8078 |
| light | home.empty | 0.8089 |
| hc | gst.recon-empty | 0.8099 |
| light | bank.import-error | 0.8104 |
| light | profit-and-loss.loading | 0.8112 |
| hc | ai.purchase-bill.error | 0.8138 |
| hc | day-book.empty | 0.8140 |
| hc | setup.ai | 0.8148 |
| dark | bank.empty | 0.8159 |
| light | bank.empty | 0.8163 |
| hc | ai.purchase-bill.choose | 0.8165 |
| hc | bank-chooser.default | 0.8169 |
| dark | gst.recon-locked | 0.8181 |
| light | gst.recon-locked | 0.8190 |
| dark | setup.gstin-error | 0.8204 |
| hc | review.empty | 0.8207 |
| light | balance-sheet.loading | 0.8214 |
| light | setup.gstin-error | 0.8223 |
| light | audit.integrity | 0.8234 |
| hc | gst.recon-error | 0.8234 |
| light | stock-summary.loading | 0.8255 |
| hc | setup.keyboard | 0.8257 |
| light | masters.create-item | 0.8259 |
| light | outstanding.loading | 0.8266 |
| hc | gst.gstr3b-unsupported | 0.8268 |
| hc | setup.welcome | 0.8272 |
| light | trial-balance.loading | 0.8273 |
| dark | profit-and-loss.loading | 0.8275 |
| dark | masters.create-item | 0.8295 |
| dark | audit.integrity | 0.8303 |
| hc | backup.updates-off | 0.8307 |
| dark | home.loading | 0.8308 |
| light | audit.unsupported | 0.8320 |
| dark | masters.items-empty | 0.8320 |
| dark | audit.unsupported | 0.8321 |
| hc | setup.rail | 0.8330 |
| dark | masters.empty | 0.8334 |
| light | ai.purchase-bill.choose | 0.8342 |
| hc | hi.setup.welcome | 0.8347 |
| light | gst.recon-empty | 0.8350 |
| light | masters.items-empty | 0.8354 |
| dark | gst.recon-empty | 0.8355 |
| light | masters.empty | 0.8364 |
| dark | balance-sheet.loading | 0.8365 |
| dark | ai.purchase-bill.choose | 0.8371 |
| dark | review.empty | 0.8382 |
| dark | audit.loading | 0.8390 |
| dark | stock-summary.loading | 0.8414 |
| light | day-book.empty | 0.8420 |
| dark | outstanding.loading | 0.8424 |
| light | review.empty | 0.8424 |
| dark | day-book.empty | 0.8428 |
| dark | trial-balance.loading | 0.8431 |
| light | setup.ai | 0.8451 |
| dark | setup.ai | 0.8468 |
| dark | ai.purchase-bill.error | 0.8475 |
| dark | gst.recon-error | 0.8482 |
| light | setup.welcome | 0.8484 |
| dark | setup.keyboard | 0.8496 |
| light | ai.purchase-bill.error | 0.8497 |
| light | setup.keyboard | 0.8498 |
| light | bank-chooser.default | 0.8510 |
| hc | start.recent | 0.8517 |
| dark | bank-chooser.default | 0.8519 |
| dark | gst.gstr3b-unsupported | 0.8521 |
| light | gst.gstr3b-unsupported | 0.8522 |
| dark | setup.welcome | 0.8524 |
| light | gst.recon-error | 0.8531 |
| dark | backup.updates-off | 0.8537 |
| light | backup.updates-off | 0.8538 |
| hc | gst.gstr1-not-loaded | 0.8553 |
| light | hi.setup.welcome | 0.8557 |
| light | setup.rail | 0.8562 |
| hc | setup.backup | 0.8568 |
| hc | gst.gstr3b-not-loaded | 0.8572 |
| dark | hi.setup.welcome | 0.8592 |
| dark | setup.rail | 0.8609 |
| light | setup.backup | 0.8645 |
| light | gst.gstr1-not-loaded | 0.8672 |
| light | gst.gstr3b-not-loaded | 0.8676 |
| dark | setup.backup | 0.8677 |
| dark | gst.gstr1-not-loaded | 0.8685 |
| dark | gst.gstr3b-not-loaded | 0.8690 |
| hc | setup.done | 0.8700 |
| dark | start.recent | 0.8725 |
| light | start.recent | 0.8732 |
| light | setup.done | 0.8966 |
| dark | setup.done | 0.8984 |

## hc / import.file-error

SSIM 0.4249

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.file-error.webp" width="480" alt="mock hc import.file-error"></td>
<td><img src="app-captures/hc/import.file-error.webp" width="480" alt="app hc import.file-error"></td>
</tr>
</table>

## hc / import.file-transactions

SSIM 0.4363

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.file-transactions.webp" width="480" alt="mock hc import.file-transactions"></td>
<td><img src="app-captures/hc/import.file-transactions.webp" width="480" alt="app hc import.file-transactions"></td>
</tr>
</table>

## hc / import.file-overwritten

SSIM 0.4396

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.file-overwritten.webp" width="480" alt="mock hc import.file-overwritten"></td>
<td><img src="app-captures/hc/import.file-overwritten.webp" width="480" alt="app hc import.file-overwritten"></td>
</tr>
</table>

## hc / import.file-masters

SSIM 0.4512

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.file-masters.webp" width="480" alt="mock hc import.file-masters"></td>
<td><img src="app-captures/hc/import.file-masters.webp" width="480" alt="app hc import.file-masters"></td>
</tr>
</table>

## hc / import.file-imported

SSIM 0.4630

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.file-imported.webp" width="480" alt="mock hc import.file-imported"></td>
<td><img src="app-captures/hc/import.file-imported.webp" width="480" alt="app hc import.file-imported"></td>
</tr>
</table>

## hc / import.file-backed-up

SSIM 0.4660

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.file-backed-up.webp" width="480" alt="mock hc import.file-backed-up"></td>
<td><img src="app-captures/hc/import.file-backed-up.webp" width="480" alt="app hc import.file-backed-up"></td>
</tr>
</table>

## hc / hi.import.file-overwritten

SSIM 0.4675

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.import.file-overwritten.webp" width="480" alt="mock hc hi.import.file-overwritten"></td>
<td><img src="app-captures/hc/hi.import.file-overwritten.webp" width="480" alt="app hc hi.import.file-overwritten"></td>
</tr>
</table>

## hc / hi.import.file-transactions

SSIM 0.4684

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.import.file-transactions.webp" width="480" alt="mock hc hi.import.file-transactions"></td>
<td><img src="app-captures/hc/hi.import.file-transactions.webp" width="480" alt="app hc hi.import.file-transactions"></td>
</tr>
</table>

## hc / hi.import.file-masters

SSIM 0.4712

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.import.file-masters.webp" width="480" alt="mock hc hi.import.file-masters"></td>
<td><img src="app-captures/hc/hi.import.file-masters.webp" width="480" alt="app hc hi.import.file-masters"></td>
</tr>
</table>

## hc / hi.import.file-imported

SSIM 0.4713

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.import.file-imported.webp" width="480" alt="mock hc hi.import.file-imported"></td>
<td><img src="app-captures/hc/hi.import.file-imported.webp" width="480" alt="app hc hi.import.file-imported"></td>
</tr>
</table>

## hc / hi.import.file-error

SSIM 0.4733

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.import.file-error.webp" width="480" alt="mock hc hi.import.file-error"></td>
<td><img src="app-captures/hc/hi.import.file-error.webp" width="480" alt="app hc hi.import.file-error"></td>
</tr>
</table>

## hc / import.file-missing-choice

SSIM 0.4766

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.file-missing-choice.webp" width="480" alt="mock hc import.file-missing-choice"></td>
<td><img src="app-captures/hc/import.file-missing-choice.webp" width="480" alt="app hc import.file-missing-choice"></td>
</tr>
</table>

## hc / sales-invoice.save-confirm

SSIM 0.4793

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.save-confirm.webp" width="480" alt="mock hc sales-invoice.save-confirm"></td>
<td><img src="app-captures/hc/sales-invoice.save-confirm.webp" width="480" alt="app hc sales-invoice.save-confirm"></td>
</tr>
</table>

## hc / hi.import.file-backed-up

SSIM 0.4815

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.import.file-backed-up.webp" width="480" alt="mock hc hi.import.file-backed-up"></td>
<td><img src="app-captures/hc/hi.import.file-backed-up.webp" width="480" alt="app hc hi.import.file-backed-up"></td>
</tr>
</table>

## hc / profit-and-loss.print-config

SSIM 0.4926

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.print-config.webp" width="480" alt="mock hc profit-and-loss.print-config"></td>
<td><img src="app-captures/hc/profit-and-loss.print-config.webp" width="480" alt="app hc profit-and-loss.print-config"></td>
</tr>
</table>

## hc / backup.restore

SSIM 0.5082

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/backup.restore.webp" width="480" alt="mock hc backup.restore"></td>
<td><img src="app-captures/hc/backup.restore.webp" width="480" alt="app hc backup.restore"></td>
</tr>
</table>

## hc / audit.default

SSIM 0.5163

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/audit.default.webp" width="480" alt="mock hc audit.default"></td>
<td><img src="app-captures/hc/audit.default.webp" width="480" alt="app hc audit.default"></td>
</tr>
</table>

## hc / day-book.month

SSIM 0.5180

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/day-book.month.webp" width="480" alt="mock hc day-book.month"></td>
<td><img src="app-captures/hc/day-book.month.webp" width="480" alt="app hc day-book.month"></td>
</tr>
</table>

## hc / profit-and-loss.print-failed

SSIM 0.5193

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.print-failed.webp" width="480" alt="mock hc profit-and-loss.print-failed"></td>
<td><img src="app-captures/hc/profit-and-loss.print-failed.webp" width="480" alt="app hc profit-and-loss.print-failed"></td>
</tr>
</table>

## dark / profit-and-loss.print-config

SSIM 0.5209

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.print-config.webp" width="480" alt="mock dark profit-and-loss.print-config"></td>
<td><img src="app-captures/dark/profit-and-loss.print-config.webp" width="480" alt="app dark profit-and-loss.print-config"></td>
</tr>
</table>

## hc / masters.loading

SSIM 0.5231

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.loading.webp" width="480" alt="mock hc masters.loading"></td>
<td><img src="app-captures/hc/masters.loading.webp" width="480" alt="app hc masters.loading"></td>
</tr>
</table>

## hc / profit-and-loss.provisional

SSIM 0.5268

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.provisional.webp" width="480" alt="mock hc profit-and-loss.provisional"></td>
<td><img src="app-captures/hc/profit-and-loss.provisional.webp" width="480" alt="app hc profit-and-loss.provisional"></td>
</tr>
</table>

## hc / ai.ai-settings.local-downloading

SSIM 0.5306

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.ai-settings.local-downloading.webp" width="480" alt="mock hc ai.ai-settings.local-downloading"></td>
<td><img src="app-captures/hc/ai.ai-settings.local-downloading.webp" width="480" alt="app hc ai.ai-settings.local-downloading"></td>
</tr>
</table>

## hc / sales-invoice.as-voucher

SSIM 0.5308

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.as-voucher.webp" width="480" alt="mock hc sales-invoice.as-voucher"></td>
<td><img src="app-captures/hc/sales-invoice.as-voucher.webp" width="480" alt="app hc sales-invoice.as-voucher"></td>
</tr>
</table>

## hc / ai.ai-settings.local-ready

SSIM 0.5311

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.ai-settings.local-ready.webp" width="480" alt="mock hc ai.ai-settings.local-ready"></td>
<td><img src="app-captures/hc/ai.ai-settings.local-ready.webp" width="480" alt="app hc ai.ai-settings.local-ready"></td>
</tr>
</table>

## hc / profit-and-loss.detailed

SSIM 0.5316

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.detailed.webp" width="480" alt="mock hc profit-and-loss.detailed"></td>
<td><img src="app-captures/hc/profit-and-loss.detailed.webp" width="480" alt="app hc profit-and-loss.detailed"></td>
</tr>
</table>

## hc / hi.import.file-missing-choice

SSIM 0.5331

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.import.file-missing-choice.webp" width="480" alt="mock hc hi.import.file-missing-choice"></td>
<td><img src="app-captures/hc/hi.import.file-missing-choice.webp" width="480" alt="app hc hi.import.file-missing-choice"></td>
</tr>
</table>

## hc / home.nudge-open

SSIM 0.5350

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/home.nudge-open.webp" width="480" alt="mock hc home.nudge-open"></td>
<td><img src="app-captures/hc/home.nudge-open.webp" width="480" alt="app hc home.nudge-open"></td>
</tr>
</table>

## hc / home.gst-unsupported

SSIM 0.5479

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/home.gst-unsupported.webp" width="480" alt="mock hc home.gst-unsupported"></td>
<td><img src="app-captures/hc/home.gst-unsupported.webp" width="480" alt="app hc home.gst-unsupported"></td>
</tr>
</table>

## hc / import.port-error

SSIM 0.5501

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.port-error.webp" width="480" alt="mock hc import.port-error"></td>
<td><img src="app-captures/hc/import.port-error.webp" width="480" alt="app hc import.port-error"></td>
</tr>
</table>

## hc / import.validation-error

SSIM 0.5511

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.validation-error.webp" width="480" alt="mock hc import.validation-error"></td>
<td><img src="app-captures/hc/import.validation-error.webp" width="480" alt="app hc import.validation-error"></td>
</tr>
</table>

## hc / ai.purchase-bill.needs-review

SSIM 0.5525

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.purchase-bill.needs-review.webp" width="480" alt="mock hc ai.purchase-bill.needs-review"></td>
<td><img src="app-captures/hc/ai.purchase-bill.needs-review.webp" width="480" alt="app hc ai.purchase-bill.needs-review"></td>
</tr>
</table>

## hc / bank.default

SSIM 0.5526

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/bank.default.webp" width="480" alt="mock hc bank.default"></td>
<td><img src="app-captures/hc/bank.default.webp" width="480" alt="app hc bank.default"></td>
</tr>
</table>

## hc / bank.offline-ai

SSIM 0.5526

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/bank.offline-ai.webp" width="480" alt="mock hc bank.offline-ai"></td>
<td><img src="app-captures/hc/bank.offline-ai.webp" width="480" alt="app hc bank.offline-ai"></td>
</tr>
</table>

## hc / ai.review.default

SSIM 0.5536

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.review.default.webp" width="480" alt="mock hc ai.review.default"></td>
<td><img src="app-captures/hc/ai.review.default.webp" width="480" alt="app hc ai.review.default"></td>
</tr>
</table>

## hc / review.default

SSIM 0.5537

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/review.default.webp" width="480" alt="mock hc review.default"></td>
<td><img src="app-captures/hc/review.default.webp" width="480" alt="app hc review.default"></td>
</tr>
</table>

## hc / import.imported

SSIM 0.5544

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.imported.webp" width="480" alt="mock hc import.imported"></td>
<td><img src="app-captures/hc/import.imported.webp" width="480" alt="app hc import.imported"></td>
</tr>
</table>

## hc / stock-summary.provisional

SSIM 0.5556

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/stock-summary.provisional.webp" width="480" alt="mock hc stock-summary.provisional"></td>
<td><img src="app-captures/hc/stock-summary.provisional.webp" width="480" alt="app hc stock-summary.provisional"></td>
</tr>
</table>

## dark / import.file-transactions

SSIM 0.5559

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.file-transactions.webp" width="480" alt="mock dark import.file-transactions"></td>
<td><img src="app-captures/dark/import.file-transactions.webp" width="480" alt="app dark import.file-transactions"></td>
</tr>
</table>

## hc / sales-invoice.einvoice-not-ready

SSIM 0.5598

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.einvoice-not-ready.webp" width="480" alt="mock hc sales-invoice.einvoice-not-ready"></td>
<td><img src="app-captures/hc/sales-invoice.einvoice-not-ready.webp" width="480" alt="app hc sales-invoice.einvoice-not-ready"></td>
</tr>
</table>

## dark / import.file-masters

SSIM 0.5599

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.file-masters.webp" width="480" alt="mock dark import.file-masters"></td>
<td><img src="app-captures/dark/import.file-masters.webp" width="480" alt="app dark import.file-masters"></td>
</tr>
</table>

## hc / import.no-company

SSIM 0.5614

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.no-company.webp" width="480" alt="mock hc import.no-company"></td>
<td><img src="app-captures/hc/import.no-company.webp" width="480" alt="app hc import.no-company"></td>
</tr>
</table>

## hc / hi.bank.default

SSIM 0.5637

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.bank.default.webp" width="480" alt="mock hc hi.bank.default"></td>
<td><img src="app-captures/hc/hi.bank.default.webp" width="480" alt="app hc hi.bank.default"></td>
</tr>
</table>

## light / import.file-transactions

SSIM 0.5640

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.file-transactions.webp" width="480" alt="mock light import.file-transactions"></td>
<td><img src="app-captures/light/import.file-transactions.webp" width="480" alt="app light import.file-transactions"></td>
</tr>
</table>

## hc / backup.backed-up

SSIM 0.5648

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/backup.backed-up.webp" width="480" alt="mock hc backup.backed-up"></td>
<td><img src="app-captures/hc/backup.backed-up.webp" width="480" alt="app hc backup.backed-up"></td>
</tr>
</table>

## hc / gst.gstr3b

SSIM 0.5649

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.gstr3b.webp" width="480" alt="mock hc gst.gstr3b"></td>
<td><img src="app-captures/hc/gst.gstr3b.webp" width="480" alt="app hc gst.gstr3b"></td>
</tr>
</table>

## light / import.file-masters

SSIM 0.5656

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.file-masters.webp" width="480" alt="mock light import.file-masters"></td>
<td><img src="app-captures/light/import.file-masters.webp" width="480" alt="app light import.file-masters"></td>
</tr>
</table>

## hc / profit-and-loss.print-range-error

SSIM 0.5657

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.print-range-error.webp" width="480" alt="mock hc profit-and-loss.print-range-error"></td>
<td><img src="app-captures/hc/profit-and-loss.print-range-error.webp" width="480" alt="app hc profit-and-loss.print-range-error"></td>
</tr>
</table>

## dark / profit-and-loss.print-range-error

SSIM 0.5667

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.print-range-error.webp" width="480" alt="mock dark profit-and-loss.print-range-error"></td>
<td><img src="app-captures/dark/profit-and-loss.print-range-error.webp" width="480" alt="app dark profit-and-loss.print-range-error"></td>
</tr>
</table>

## hc / import.failed

SSIM 0.5694

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.failed.webp" width="480" alt="mock hc import.failed"></td>
<td><img src="app-captures/hc/import.failed.webp" width="480" alt="app hc import.failed"></td>
</tr>
</table>

## hc / import.refused

SSIM 0.5702

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.refused.webp" width="480" alt="mock hc import.refused"></td>
<td><img src="app-captures/hc/import.refused.webp" width="480" alt="app hc import.refused"></td>
</tr>
</table>

## hc / ai.quick-entry.proposal

SSIM 0.5709

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.quick-entry.proposal.webp" width="480" alt="mock hc ai.quick-entry.proposal"></td>
<td><img src="app-captures/hc/ai.quick-entry.proposal.webp" width="480" alt="app hc ai.quick-entry.proposal"></td>
</tr>
</table>

## dark / import.file-overwritten

SSIM 0.5713

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.file-overwritten.webp" width="480" alt="mock dark import.file-overwritten"></td>
<td><img src="app-captures/dark/import.file-overwritten.webp" width="480" alt="app dark import.file-overwritten"></td>
</tr>
</table>

## light / import.file-overwritten

SSIM 0.5717

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.file-overwritten.webp" width="480" alt="mock light import.file-overwritten"></td>
<td><img src="app-captures/light/import.file-overwritten.webp" width="480" alt="app light import.file-overwritten"></td>
</tr>
</table>

## hc / trial-balance.default

SSIM 0.5751

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/trial-balance.default.webp" width="480" alt="mock hc trial-balance.default"></td>
<td><img src="app-captures/hc/trial-balance.default.webp" width="480" alt="app hc trial-balance.default"></td>
</tr>
</table>

## dark / hi.import.file-transactions

SSIM 0.5772

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.import.file-transactions.webp" width="480" alt="mock dark hi.import.file-transactions"></td>
<td><img src="app-captures/dark/hi.import.file-transactions.webp" width="480" alt="app dark hi.import.file-transactions"></td>
</tr>
</table>

## light / import.file-error

SSIM 0.5779

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.file-error.webp" width="480" alt="mock light import.file-error"></td>
<td><img src="app-captures/light/import.file-error.webp" width="480" alt="app light import.file-error"></td>
</tr>
</table>

## dark / profit-and-loss.print-preview

SSIM 0.5779

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.print-preview.webp" width="480" alt="mock dark profit-and-loss.print-preview"></td>
<td><img src="app-captures/dark/profit-and-loss.print-preview.webp" width="480" alt="app dark profit-and-loss.print-preview"></td>
</tr>
</table>

## hc / profit-and-loss.print-preview

SSIM 0.5780

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.print-preview.webp" width="480" alt="mock hc profit-and-loss.print-preview"></td>
<td><img src="app-captures/hc/profit-and-loss.print-preview.webp" width="480" alt="app hc profit-and-loss.print-preview"></td>
</tr>
</table>

## hc / ai-settings.off

SSIM 0.5786

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai-settings.off.webp" width="480" alt="mock hc ai-settings.off"></td>
<td><img src="app-captures/hc/ai-settings.off.webp" width="480" alt="app hc ai-settings.off"></td>
</tr>
</table>

## hc / ai.ai-settings.cloud

SSIM 0.5792

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.ai-settings.cloud.webp" width="480" alt="mock hc ai.ai-settings.cloud"></td>
<td><img src="app-captures/hc/ai.ai-settings.cloud.webp" width="480" alt="app hc ai.ai-settings.cloud"></td>
</tr>
</table>

## dark / import.file-error

SSIM 0.5793

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.file-error.webp" width="480" alt="mock dark import.file-error"></td>
<td><img src="app-captures/dark/import.file-error.webp" width="480" alt="app dark import.file-error"></td>
</tr>
</table>

## hc / ai.quick-entry.rejected

SSIM 0.5795

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.quick-entry.rejected.webp" width="480" alt="mock hc ai.quick-entry.rejected"></td>
<td><img src="app-captures/hc/ai.quick-entry.rejected.webp" width="480" alt="app hc ai.quick-entry.rejected"></td>
</tr>
</table>

## hc / masters.ledgers

SSIM 0.5818

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.ledgers.webp" width="480" alt="mock hc masters.ledgers"></td>
<td><img src="app-captures/hc/masters.ledgers.webp" width="480" alt="app hc masters.ledgers"></td>
</tr>
</table>

## dark / import.file-imported

SSIM 0.5823

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.file-imported.webp" width="480" alt="mock dark import.file-imported"></td>
<td><img src="app-captures/dark/import.file-imported.webp" width="480" alt="app dark import.file-imported"></td>
</tr>
</table>

## hc / ai.purchase-bill.ledger-lines

SSIM 0.5848

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.purchase-bill.ledger-lines.webp" width="480" alt="mock hc ai.purchase-bill.ledger-lines"></td>
<td><img src="app-captures/hc/ai.purchase-bill.ledger-lines.webp" width="480" alt="app hc ai.purchase-bill.ledger-lines"></td>
</tr>
</table>

## light / hi.import.file-transactions

SSIM 0.5862

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.import.file-transactions.webp" width="480" alt="mock light hi.import.file-transactions"></td>
<td><img src="app-captures/light/hi.import.file-transactions.webp" width="480" alt="app light hi.import.file-transactions"></td>
</tr>
</table>

## hc / ai.ai-settings.local-download-failed

SSIM 0.5874

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.ai-settings.local-download-failed.webp" width="480" alt="mock hc ai.ai-settings.local-download-failed"></td>
<td><img src="app-captures/hc/ai.ai-settings.local-download-failed.webp" width="480" alt="app hc ai.ai-settings.local-download-failed"></td>
</tr>
</table>

## light / import.file-imported

SSIM 0.5883

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.file-imported.webp" width="480" alt="mock light import.file-imported"></td>
<td><img src="app-captures/light/import.file-imported.webp" width="480" alt="app light import.file-imported"></td>
</tr>
</table>

## dark / hi.import.file-masters

SSIM 0.5910

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.import.file-masters.webp" width="480" alt="mock dark hi.import.file-masters"></td>
<td><img src="app-captures/dark/hi.import.file-masters.webp" width="480" alt="app dark hi.import.file-masters"></td>
</tr>
</table>

## hc / import.default

SSIM 0.5916

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.default.webp" width="480" alt="mock hc import.default"></td>
<td><img src="app-captures/hc/import.default.webp" width="480" alt="app hc import.default"></td>
</tr>
</table>

## hc / sales-invoice.item-list

SSIM 0.5917

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.item-list.webp" width="480" alt="mock hc sales-invoice.item-list"></td>
<td><img src="app-captures/hc/sales-invoice.item-list.webp" width="480" alt="app hc sales-invoice.item-list"></td>
</tr>
</table>

## hc / import.running

SSIM 0.5917

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/import.running.webp" width="480" alt="mock hc import.running"></td>
<td><img src="app-captures/hc/import.running.webp" width="480" alt="app hc import.running"></td>
</tr>
</table>

## hc / bank.brs-exported

SSIM 0.5922

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/bank.brs-exported.webp" width="480" alt="mock hc bank.brs-exported"></td>
<td><img src="app-captures/hc/bank.brs-exported.webp" width="480" alt="app hc bank.brs-exported"></td>
</tr>
</table>

## light / audit.default

SSIM 0.5922

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/audit.default.webp" width="480" alt="mock light audit.default"></td>
<td><img src="app-captures/light/audit.default.webp" width="480" alt="app light audit.default"></td>
</tr>
</table>

## hc / purchase.validation-error

SSIM 0.5928

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/purchase.validation-error.webp" width="480" alt="mock hc purchase.validation-error"></td>
<td><img src="app-captures/hc/purchase.validation-error.webp" width="480" alt="app hc purchase.validation-error"></td>
</tr>
</table>

## hc / home.ai-off

SSIM 0.5928

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/home.ai-off.webp" width="480" alt="mock hc home.ai-off"></td>
<td><img src="app-captures/hc/home.ai-off.webp" width="480" alt="app hc home.ai-off"></td>
</tr>
</table>

## hc / home.default

SSIM 0.5928

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/home.default.webp" width="480" alt="mock hc home.default"></td>
<td><img src="app-captures/hc/home.default.webp" width="480" alt="app hc home.default"></td>
</tr>
</table>

## hc / hi.masters.ledgers

SSIM 0.5939

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.masters.ledgers.webp" width="480" alt="mock hc hi.masters.ledgers"></td>
<td><img src="app-captures/hc/hi.masters.ledgers.webp" width="480" alt="app hc hi.masters.ledgers"></td>
</tr>
</table>

## dark / hi.import.file-backed-up

SSIM 0.5970

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.import.file-backed-up.webp" width="480" alt="mock dark hi.import.file-backed-up"></td>
<td><img src="app-captures/dark/hi.import.file-backed-up.webp" width="480" alt="app dark hi.import.file-backed-up"></td>
</tr>
</table>

## dark / import.file-backed-up

SSIM 0.5971

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.file-backed-up.webp" width="480" alt="mock dark import.file-backed-up"></td>
<td><img src="app-captures/dark/import.file-backed-up.webp" width="480" alt="app dark import.file-backed-up"></td>
</tr>
</table>

## light / ai.purchase-bill.needs-review

SSIM 0.5981

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.purchase-bill.needs-review.webp" width="480" alt="mock light ai.purchase-bill.needs-review"></td>
<td><img src="app-captures/light/ai.purchase-bill.needs-review.webp" width="480" alt="app light ai.purchase-bill.needs-review"></td>
</tr>
</table>

## light / hi.import.file-masters

SSIM 0.5982

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.import.file-masters.webp" width="480" alt="mock light hi.import.file-masters"></td>
<td><img src="app-captures/light/hi.import.file-masters.webp" width="480" alt="app light hi.import.file-masters"></td>
</tr>
</table>

## light / import.file-backed-up

SSIM 0.5990

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.file-backed-up.webp" width="480" alt="mock light import.file-backed-up"></td>
<td><img src="app-captures/light/import.file-backed-up.webp" width="480" alt="app light import.file-backed-up"></td>
</tr>
</table>

## hc / home.new-tab

SSIM 0.6005

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/home.new-tab.webp" width="480" alt="mock hc home.new-tab"></td>
<td><img src="app-captures/hc/home.new-tab.webp" width="480" alt="app hc home.new-tab"></td>
</tr>
</table>

## hc / ai.ask.result

SSIM 0.6008

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.ask.result.webp" width="480" alt="mock hc ai.ask.result"></td>
<td><img src="app-captures/hc/ai.ask.result.webp" width="480" alt="app hc ai.ask.result"></td>
</tr>
</table>

## hc / sales-invoice.period-locked

SSIM 0.6012

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.period-locked.webp" width="480" alt="mock hc sales-invoice.period-locked"></td>
<td><img src="app-captures/hc/sales-invoice.period-locked.webp" width="480" alt="app hc sales-invoice.period-locked"></td>
</tr>
</table>

## hc / keys.standard

SSIM 0.6014

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/keys.standard.webp" width="480" alt="mock hc keys.standard"></td>
<td><img src="app-captures/hc/keys.standard.webp" width="480" alt="app hc keys.standard"></td>
</tr>
</table>

## light / import.file-missing-choice

SSIM 0.6019

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.file-missing-choice.webp" width="480" alt="mock light import.file-missing-choice"></td>
<td><img src="app-captures/light/import.file-missing-choice.webp" width="480" alt="app light import.file-missing-choice"></td>
</tr>
</table>

## hc / sales-invoice.validation-error

SSIM 0.6024

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.validation-error.webp" width="480" alt="mock hc sales-invoice.validation-error"></td>
<td><img src="app-captures/hc/sales-invoice.validation-error.webp" width="480" alt="app hc sales-invoice.validation-error"></td>
</tr>
</table>

## dark / backup.restore

SSIM 0.6025

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/backup.restore.webp" width="480" alt="mock dark backup.restore"></td>
<td><img src="app-captures/dark/backup.restore.webp" width="480" alt="app dark backup.restore"></td>
</tr>
</table>

## hc / balance-sheet.provisional

SSIM 0.6031

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/balance-sheet.provisional.webp" width="480" alt="mock hc balance-sheet.provisional"></td>
<td><img src="app-captures/hc/balance-sheet.provisional.webp" width="480" alt="app hc balance-sheet.provisional"></td>
</tr>
</table>

## dark / ai.purchase-bill.needs-review

SSIM 0.6034

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.purchase-bill.needs-review.webp" width="480" alt="mock dark ai.purchase-bill.needs-review"></td>
<td><img src="app-captures/dark/ai.purchase-bill.needs-review.webp" width="480" alt="app dark ai.purchase-bill.needs-review"></td>
</tr>
</table>

## light / hi.import.file-backed-up

SSIM 0.6037

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.import.file-backed-up.webp" width="480" alt="mock light hi.import.file-backed-up"></td>
<td><img src="app-captures/light/hi.import.file-backed-up.webp" width="480" alt="app light hi.import.file-backed-up"></td>
</tr>
</table>

## hc / ai.ai-settings.cloud-edit

SSIM 0.6040

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.ai-settings.cloud-edit.webp" width="480" alt="mock hc ai.ai-settings.cloud-edit"></td>
<td><img src="app-captures/hc/ai.ai-settings.cloud-edit.webp" width="480" alt="app hc ai.ai-settings.cloud-edit"></td>
</tr>
</table>

## hc / ai.quick-entry.unknown-ledger

SSIM 0.6044

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.quick-entry.unknown-ledger.webp" width="480" alt="mock hc ai.quick-entry.unknown-ledger"></td>
<td><img src="app-captures/hc/ai.quick-entry.unknown-ledger.webp" width="480" alt="app hc ai.quick-entry.unknown-ledger"></td>
</tr>
</table>

## light / ai.review.default

SSIM 0.6046

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.review.default.webp" width="480" alt="mock light ai.review.default"></td>
<td><img src="app-captures/light/ai.review.default.webp" width="480" alt="app light ai.review.default"></td>
</tr>
</table>

## light / backup.restore

SSIM 0.6064

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/backup.restore.webp" width="480" alt="mock light backup.restore"></td>
<td><img src="app-captures/light/backup.restore.webp" width="480" alt="app light backup.restore"></td>
</tr>
</table>

## light / ai.ai-settings.local-downloading

SSIM 0.6067

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.ai-settings.local-downloading.webp" width="480" alt="mock light ai.ai-settings.local-downloading"></td>
<td><img src="app-captures/light/ai.ai-settings.local-downloading.webp" width="480" alt="app light ai.ai-settings.local-downloading"></td>
</tr>
</table>

## dark / hi.import.file-overwritten

SSIM 0.6075

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.import.file-overwritten.webp" width="480" alt="mock dark hi.import.file-overwritten"></td>
<td><img src="app-captures/dark/hi.import.file-overwritten.webp" width="480" alt="app dark hi.import.file-overwritten"></td>
</tr>
</table>

## hc / sales-invoice.mode-list

SSIM 0.6075

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.mode-list.webp" width="480" alt="mock hc sales-invoice.mode-list"></td>
<td><img src="app-captures/hc/sales-invoice.mode-list.webp" width="480" alt="app hc sales-invoice.mode-list"></td>
</tr>
</table>

## light / ai.ai-settings.local-ready

SSIM 0.6078

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.ai-settings.local-ready.webp" width="480" alt="mock light ai.ai-settings.local-ready"></td>
<td><img src="app-captures/light/ai.ai-settings.local-ready.webp" width="480" alt="app light ai.ai-settings.local-ready"></td>
</tr>
</table>

## light / review.default

SSIM 0.6078

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/review.default.webp" width="480" alt="mock light review.default"></td>
<td><img src="app-captures/light/review.default.webp" width="480" alt="app light review.default"></td>
</tr>
</table>

## light / masters.ledgers

SSIM 0.6078

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.ledgers.webp" width="480" alt="mock light masters.ledgers"></td>
<td><img src="app-captures/light/masters.ledgers.webp" width="480" alt="app light masters.ledgers"></td>
</tr>
</table>

## light / sales-invoice.save-confirm

SSIM 0.6083

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.save-confirm.webp" width="480" alt="mock light sales-invoice.save-confirm"></td>
<td><img src="app-captures/light/sales-invoice.save-confirm.webp" width="480" alt="app light sales-invoice.save-confirm"></td>
</tr>
</table>

## hc / balance-sheet.detailed

SSIM 0.6084

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/balance-sheet.detailed.webp" width="480" alt="mock hc balance-sheet.detailed"></td>
<td><img src="app-captures/hc/balance-sheet.detailed.webp" width="480" alt="app hc balance-sheet.detailed"></td>
</tr>
</table>

## dark / audit.default

SSIM 0.6086

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/audit.default.webp" width="480" alt="mock dark audit.default"></td>
<td><img src="app-captures/dark/audit.default.webp" width="480" alt="app dark audit.default"></td>
</tr>
</table>

## dark / review.default

SSIM 0.6091

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/review.default.webp" width="480" alt="mock dark review.default"></td>
<td><img src="app-captures/dark/review.default.webp" width="480" alt="app dark review.default"></td>
</tr>
</table>

## hc / gst.gstr1-correction

SSIM 0.6094

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.gstr1-correction.webp" width="480" alt="mock hc gst.gstr1-correction"></td>
<td><img src="app-captures/hc/gst.gstr1-correction.webp" width="480" alt="app hc gst.gstr1-correction"></td>
</tr>
</table>

## hc / ai.ai-settings.local

SSIM 0.6095

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.ai-settings.local.webp" width="480" alt="mock hc ai.ai-settings.local"></td>
<td><img src="app-captures/hc/ai.ai-settings.local.webp" width="480" alt="app hc ai.ai-settings.local"></td>
</tr>
</table>

## hc / gst.gstr1

SSIM 0.6095

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.gstr1.webp" width="480" alt="mock hc gst.gstr1"></td>
<td><img src="app-captures/hc/gst.gstr1.webp" width="480" alt="app hc gst.gstr1"></td>
</tr>
</table>

## dark / ai.review.default

SSIM 0.6098

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.review.default.webp" width="480" alt="mock dark ai.review.default"></td>
<td><img src="app-captures/dark/ai.review.default.webp" width="480" alt="app dark ai.review.default"></td>
</tr>
</table>

## dark / bank.default

SSIM 0.6106

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/bank.default.webp" width="480" alt="mock dark bank.default"></td>
<td><img src="app-captures/dark/bank.default.webp" width="480" alt="app dark bank.default"></td>
</tr>
</table>

## dark / bank.offline-ai

SSIM 0.6106

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/bank.offline-ai.webp" width="480" alt="mock dark bank.offline-ai"></td>
<td><img src="app-captures/dark/bank.offline-ai.webp" width="480" alt="app dark bank.offline-ai"></td>
</tr>
</table>

## light / hi.import.file-overwritten

SSIM 0.6110

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.import.file-overwritten.webp" width="480" alt="mock light hi.import.file-overwritten"></td>
<td><img src="app-captures/light/hi.import.file-overwritten.webp" width="480" alt="app light hi.import.file-overwritten"></td>
</tr>
</table>

## hc / keys.classic

SSIM 0.6117

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/keys.classic.webp" width="480" alt="mock hc keys.classic"></td>
<td><img src="app-captures/hc/keys.classic.webp" width="480" alt="app hc keys.classic"></td>
</tr>
</table>

## hc / ai.quick-entry.accepted

SSIM 0.6125

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.quick-entry.accepted.webp" width="480" alt="mock hc ai.quick-entry.accepted"></td>
<td><img src="app-captures/hc/ai.quick-entry.accepted.webp" width="480" alt="app hc ai.quick-entry.accepted"></td>
</tr>
</table>

## light / gst.gstr3b

SSIM 0.6126

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.gstr3b.webp" width="480" alt="mock light gst.gstr3b"></td>
<td><img src="app-captures/light/gst.gstr3b.webp" width="480" alt="app light gst.gstr3b"></td>
</tr>
</table>

## hc / masters.items

SSIM 0.6133

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.items.webp" width="480" alt="mock hc masters.items"></td>
<td><img src="app-captures/hc/masters.items.webp" width="480" alt="app hc masters.items"></td>
</tr>
</table>

## hc / profit-and-loss.print-busy

SSIM 0.6143

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.print-busy.webp" width="480" alt="mock hc profit-and-loss.print-busy"></td>
<td><img src="app-captures/hc/profit-and-loss.print-busy.webp" width="480" alt="app hc profit-and-loss.print-busy"></td>
</tr>
</table>

## hc / balance-sheet.default

SSIM 0.6149

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/balance-sheet.default.webp" width="480" alt="mock hc balance-sheet.default"></td>
<td><img src="app-captures/hc/balance-sheet.default.webp" width="480" alt="app hc balance-sheet.default"></td>
</tr>
</table>

## hc / profit-and-loss.print-saved

SSIM 0.6177

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.print-saved.webp" width="480" alt="mock hc profit-and-loss.print-saved"></td>
<td><img src="app-captures/hc/profit-and-loss.print-saved.webp" width="480" alt="app hc profit-and-loss.print-saved"></td>
</tr>
</table>

## dark / import.file-missing-choice

SSIM 0.6182

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.file-missing-choice.webp" width="480" alt="mock dark import.file-missing-choice"></td>
<td><img src="app-captures/dark/import.file-missing-choice.webp" width="480" alt="app dark import.file-missing-choice"></td>
</tr>
</table>

## light / balance-sheet.provisional

SSIM 0.6184

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/balance-sheet.provisional.webp" width="480" alt="mock light balance-sheet.provisional"></td>
<td><img src="app-captures/light/balance-sheet.provisional.webp" width="480" alt="app light balance-sheet.provisional"></td>
</tr>
</table>

## light / hi.masters.ledgers

SSIM 0.6190

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.masters.ledgers.webp" width="480" alt="mock light hi.masters.ledgers"></td>
<td><img src="app-captures/light/hi.masters.ledgers.webp" width="480" alt="app light hi.masters.ledgers"></td>
</tr>
</table>

## dark / hi.import.file-imported

SSIM 0.6191

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.import.file-imported.webp" width="480" alt="mock dark hi.import.file-imported"></td>
<td><img src="app-captures/dark/hi.import.file-imported.webp" width="480" alt="app dark hi.import.file-imported"></td>
</tr>
</table>

## hc / sales-invoice.gst-inter-state

SSIM 0.6199

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.gst-inter-state.webp" width="480" alt="mock hc sales-invoice.gst-inter-state"></td>
<td><img src="app-captures/hc/sales-invoice.gst-inter-state.webp" width="480" alt="app hc sales-invoice.gst-inter-state"></td>
</tr>
</table>

## dark / hi.import.file-error

SSIM 0.6201

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.import.file-error.webp" width="480" alt="mock dark hi.import.file-error"></td>
<td><img src="app-captures/dark/hi.import.file-error.webp" width="480" alt="app dark hi.import.file-error"></td>
</tr>
</table>

## hc / masters.no-match

SSIM 0.6203

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.no-match.webp" width="480" alt="mock hc masters.no-match"></td>
<td><img src="app-captures/hc/masters.no-match.webp" width="480" alt="app hc masters.no-match"></td>
</tr>
</table>

## light / home.nudge-open

SSIM 0.6207

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/home.nudge-open.webp" width="480" alt="mock light home.nudge-open"></td>
<td><img src="app-captures/light/home.nudge-open.webp" width="480" alt="app light home.nudge-open"></td>
</tr>
</table>

## dark / gst.gstr3b

SSIM 0.6208

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.gstr3b.webp" width="480" alt="mock dark gst.gstr3b"></td>
<td><img src="app-captures/dark/gst.gstr3b.webp" width="480" alt="app dark gst.gstr3b"></td>
</tr>
</table>

## light / sales-invoice.einvoice-not-ready

SSIM 0.6209

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.einvoice-not-ready.webp" width="480" alt="mock light sales-invoice.einvoice-not-ready"></td>
<td><img src="app-captures/light/sales-invoice.einvoice-not-ready.webp" width="480" alt="app light sales-invoice.einvoice-not-ready"></td>
</tr>
</table>

## dark / ai.ai-settings.local-downloading

SSIM 0.6211

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.ai-settings.local-downloading.webp" width="480" alt="mock dark ai.ai-settings.local-downloading"></td>
<td><img src="app-captures/dark/ai.ai-settings.local-downloading.webp" width="480" alt="app dark ai.ai-settings.local-downloading"></td>
</tr>
</table>

## dark / sales-invoice.save-confirm

SSIM 0.6220

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.save-confirm.webp" width="480" alt="mock dark sales-invoice.save-confirm"></td>
<td><img src="app-captures/dark/sales-invoice.save-confirm.webp" width="480" alt="app dark sales-invoice.save-confirm"></td>
</tr>
</table>

## light / trial-balance.default

SSIM 0.6221

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/trial-balance.default.webp" width="480" alt="mock light trial-balance.default"></td>
<td><img src="app-captures/light/trial-balance.default.webp" width="480" alt="app light trial-balance.default"></td>
</tr>
</table>

## dark / ai.ai-settings.local-ready

SSIM 0.6222

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.ai-settings.local-ready.webp" width="480" alt="mock dark ai.ai-settings.local-ready"></td>
<td><img src="app-captures/dark/ai.ai-settings.local-ready.webp" width="480" alt="app dark ai.ai-settings.local-ready"></td>
</tr>
</table>

## light / hi.import.file-error

SSIM 0.6222

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.import.file-error.webp" width="480" alt="mock light hi.import.file-error"></td>
<td><img src="app-captures/light/hi.import.file-error.webp" width="480" alt="app light hi.import.file-error"></td>
</tr>
</table>

## light / day-book.month

SSIM 0.6222

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/day-book.month.webp" width="480" alt="mock light day-book.month"></td>
<td><img src="app-captures/light/day-book.month.webp" width="480" alt="app light day-book.month"></td>
</tr>
</table>

## light / sales-invoice.period-locked

SSIM 0.6228

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.period-locked.webp" width="480" alt="mock light sales-invoice.period-locked"></td>
<td><img src="app-captures/light/sales-invoice.period-locked.webp" width="480" alt="app light sales-invoice.period-locked"></td>
</tr>
</table>

## light / ai.purchase-bill.ledger-lines

SSIM 0.6242

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.purchase-bill.ledger-lines.webp" width="480" alt="mock light ai.purchase-bill.ledger-lines"></td>
<td><img src="app-captures/light/ai.purchase-bill.ledger-lines.webp" width="480" alt="app light ai.purchase-bill.ledger-lines"></td>
</tr>
</table>

## hc / hi.gst.gstr1

SSIM 0.6252

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.gst.gstr1.webp" width="480" alt="mock hc hi.gst.gstr1"></td>
<td><img src="app-captures/hc/hi.gst.gstr1.webp" width="480" alt="app hc hi.gst.gstr1"></td>
</tr>
</table>

## hc / sales-invoice.default

SSIM 0.6257

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.default.webp" width="480" alt="mock hc sales-invoice.default"></td>
<td><img src="app-captures/hc/sales-invoice.default.webp" width="480" alt="app hc sales-invoice.default"></td>
</tr>
</table>

## hc / hi.balance-sheet.default

SSIM 0.6258

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.balance-sheet.default.webp" width="480" alt="mock hc hi.balance-sheet.default"></td>
<td><img src="app-captures/hc/hi.balance-sheet.default.webp" width="480" alt="app hc hi.balance-sheet.default"></td>
</tr>
</table>

## dark / hi.bank.default

SSIM 0.6282

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.bank.default.webp" width="480" alt="mock dark hi.bank.default"></td>
<td><img src="app-captures/dark/hi.bank.default.webp" width="480" alt="app dark hi.bank.default"></td>
</tr>
</table>

## light / hi.import.file-imported

SSIM 0.6282

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.import.file-imported.webp" width="480" alt="mock light hi.import.file-imported"></td>
<td><img src="app-captures/light/hi.import.file-imported.webp" width="480" alt="app light hi.import.file-imported"></td>
</tr>
</table>

## dark / home.nudge-open

SSIM 0.6283

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/home.nudge-open.webp" width="480" alt="mock dark home.nudge-open"></td>
<td><img src="app-captures/dark/home.nudge-open.webp" width="480" alt="app dark home.nudge-open"></td>
</tr>
</table>

## light / profit-and-loss.detailed

SSIM 0.6295

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.detailed.webp" width="480" alt="mock light profit-and-loss.detailed"></td>
<td><img src="app-captures/light/profit-and-loss.detailed.webp" width="480" alt="app light profit-and-loss.detailed"></td>
</tr>
</table>

## hc / bank.brs

SSIM 0.6307

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/bank.brs.webp" width="480" alt="mock hc bank.brs"></td>
<td><img src="app-captures/hc/bank.brs.webp" width="480" alt="app hc bank.brs"></td>
</tr>
</table>

## light / bank.default

SSIM 0.6309

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/bank.default.webp" width="480" alt="mock light bank.default"></td>
<td><img src="app-captures/light/bank.default.webp" width="480" alt="app light bank.default"></td>
</tr>
</table>

## light / bank.offline-ai

SSIM 0.6309

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/bank.offline-ai.webp" width="480" alt="mock light bank.offline-ai"></td>
<td><img src="app-captures/light/bank.offline-ai.webp" width="480" alt="app light bank.offline-ai"></td>
</tr>
</table>

## light / sales-invoice.validation-error

SSIM 0.6316

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.validation-error.webp" width="480" alt="mock light sales-invoice.validation-error"></td>
<td><img src="app-captures/light/sales-invoice.validation-error.webp" width="480" alt="app light sales-invoice.validation-error"></td>
</tr>
</table>

## dark / ai.purchase-bill.ledger-lines

SSIM 0.6316

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.purchase-bill.ledger-lines.webp" width="480" alt="mock dark ai.purchase-bill.ledger-lines"></td>
<td><img src="app-captures/dark/ai.purchase-bill.ledger-lines.webp" width="480" alt="app dark ai.purchase-bill.ledger-lines"></td>
</tr>
</table>

## hc / ai.purchase-bill.reading

SSIM 0.6324

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.purchase-bill.reading.webp" width="480" alt="mock hc ai.purchase-bill.reading"></td>
<td><img src="app-captures/hc/ai.purchase-bill.reading.webp" width="480" alt="app hc ai.purchase-bill.reading"></td>
</tr>
</table>

## hc / sales-invoice.print-preview

SSIM 0.6330

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.print-preview.webp" width="480" alt="mock hc sales-invoice.print-preview"></td>
<td><img src="app-captures/hc/sales-invoice.print-preview.webp" width="480" alt="app hc sales-invoice.print-preview"></td>
</tr>
</table>

## dark / day-book.month

SSIM 0.6335

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/day-book.month.webp" width="480" alt="mock dark day-book.month"></td>
<td><img src="app-captures/dark/day-book.month.webp" width="480" alt="app dark day-book.month"></td>
</tr>
</table>

## dark / ai.quick-entry.proposal

SSIM 0.6335

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.quick-entry.proposal.webp" width="480" alt="mock dark ai.quick-entry.proposal"></td>
<td><img src="app-captures/dark/ai.quick-entry.proposal.webp" width="480" alt="app dark ai.quick-entry.proposal"></td>
</tr>
</table>

## hc / hi.sales-invoice.default

SSIM 0.6336

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.sales-invoice.default.webp" width="480" alt="mock hc hi.sales-invoice.default"></td>
<td><img src="app-captures/hc/hi.sales-invoice.default.webp" width="480" alt="app hc hi.sales-invoice.default"></td>
</tr>
</table>

## light / sales-invoice.default

SSIM 0.6336

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.default.webp" width="480" alt="mock light sales-invoice.default"></td>
<td><img src="app-captures/light/sales-invoice.default.webp" width="480" alt="app light sales-invoice.default"></td>
</tr>
</table>

## dark / home.gst-unsupported

SSIM 0.6340

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/home.gst-unsupported.webp" width="480" alt="mock dark home.gst-unsupported"></td>
<td><img src="app-captures/dark/home.gst-unsupported.webp" width="480" alt="app dark home.gst-unsupported"></td>
</tr>
</table>

## light / sales-invoice.as-voucher

SSIM 0.6345

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.as-voucher.webp" width="480" alt="mock light sales-invoice.as-voucher"></td>
<td><img src="app-captures/light/sales-invoice.as-voucher.webp" width="480" alt="app light sales-invoice.as-voucher"></td>
</tr>
</table>

## hc / backup.default

SSIM 0.6345

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/backup.default.webp" width="480" alt="mock hc backup.default"></td>
<td><img src="app-captures/hc/backup.default.webp" width="480" alt="app hc backup.default"></td>
</tr>
</table>

## light / sales-invoice.item-list

SSIM 0.6347

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.item-list.webp" width="480" alt="mock light sales-invoice.item-list"></td>
<td><img src="app-captures/light/sales-invoice.item-list.webp" width="480" alt="app light sales-invoice.item-list"></td>
</tr>
</table>

## light / hi.import.file-missing-choice

SSIM 0.6359

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.import.file-missing-choice.webp" width="480" alt="mock light hi.import.file-missing-choice"></td>
<td><img src="app-captures/light/hi.import.file-missing-choice.webp" width="480" alt="app light hi.import.file-missing-choice"></td>
</tr>
</table>

## light / ai.quick-entry.proposal

SSIM 0.6373

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.quick-entry.proposal.webp" width="480" alt="mock light ai.quick-entry.proposal"></td>
<td><img src="app-captures/light/ai.quick-entry.proposal.webp" width="480" alt="app light ai.quick-entry.proposal"></td>
</tr>
</table>

## dark / sales-invoice.print-preview

SSIM 0.6378

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.print-preview.webp" width="480" alt="mock dark sales-invoice.print-preview"></td>
<td><img src="app-captures/dark/sales-invoice.print-preview.webp" width="480" alt="app dark sales-invoice.print-preview"></td>
</tr>
</table>

## hc / hi.home.default

SSIM 0.6378

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.home.default.webp" width="480" alt="mock hc hi.home.default"></td>
<td><img src="app-captures/hc/hi.home.default.webp" width="480" alt="app hc hi.home.default"></td>
</tr>
</table>

## light / sales-invoice.gst-inter-state

SSIM 0.6379

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.gst-inter-state.webp" width="480" alt="mock light sales-invoice.gst-inter-state"></td>
<td><img src="app-captures/light/sales-invoice.gst-inter-state.webp" width="480" alt="app light sales-invoice.gst-inter-state"></td>
</tr>
</table>

## dark / masters.ledgers

SSIM 0.6385

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.ledgers.webp" width="480" alt="mock dark masters.ledgers"></td>
<td><img src="app-captures/dark/masters.ledgers.webp" width="480" alt="app dark masters.ledgers"></td>
</tr>
</table>

## dark / sales-invoice.einvoice-not-ready

SSIM 0.6388

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.einvoice-not-ready.webp" width="480" alt="mock dark sales-invoice.einvoice-not-ready"></td>
<td><img src="app-captures/dark/sales-invoice.einvoice-not-ready.webp" width="480" alt="app dark sales-invoice.einvoice-not-ready"></td>
</tr>
</table>

## hc / day-book.print-preview

SSIM 0.6395

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/day-book.print-preview.webp" width="480" alt="mock hc day-book.print-preview"></td>
<td><img src="app-captures/hc/day-book.print-preview.webp" width="480" alt="app hc day-book.print-preview"></td>
</tr>
</table>

## light / ai.quick-entry.rejected

SSIM 0.6396

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.quick-entry.rejected.webp" width="480" alt="mock light ai.quick-entry.rejected"></td>
<td><img src="app-captures/light/ai.quick-entry.rejected.webp" width="480" alt="app light ai.quick-entry.rejected"></td>
</tr>
</table>

## dark / ai.quick-entry.rejected

SSIM 0.6399

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.quick-entry.rejected.webp" width="480" alt="mock dark ai.quick-entry.rejected"></td>
<td><img src="app-captures/dark/ai.quick-entry.rejected.webp" width="480" alt="app dark ai.quick-entry.rejected"></td>
</tr>
</table>

## dark / day-book.print-preview

SSIM 0.6408

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/day-book.print-preview.webp" width="480" alt="mock dark day-book.print-preview"></td>
<td><img src="app-captures/dark/day-book.print-preview.webp" width="480" alt="app dark day-book.print-preview"></td>
</tr>
</table>

## light / ai.ai-settings.local-download-failed

SSIM 0.6413

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.ai-settings.local-download-failed.webp" width="480" alt="mock light ai.ai-settings.local-download-failed"></td>
<td><img src="app-captures/light/ai.ai-settings.local-download-failed.webp" width="480" alt="app light ai.ai-settings.local-download-failed"></td>
</tr>
</table>

## light / home.gst-unsupported

SSIM 0.6420

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/home.gst-unsupported.webp" width="480" alt="mock light home.gst-unsupported"></td>
<td><img src="app-captures/light/home.gst-unsupported.webp" width="480" alt="app light home.gst-unsupported"></td>
</tr>
</table>

## dark / sales-invoice.period-locked

SSIM 0.6421

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.period-locked.webp" width="480" alt="mock dark sales-invoice.period-locked"></td>
<td><img src="app-captures/dark/sales-invoice.period-locked.webp" width="480" alt="app dark sales-invoice.period-locked"></td>
</tr>
</table>

## dark / sales-invoice.validation-error

SSIM 0.6423

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.validation-error.webp" width="480" alt="mock dark sales-invoice.validation-error"></td>
<td><img src="app-captures/dark/sales-invoice.validation-error.webp" width="480" alt="app dark sales-invoice.validation-error"></td>
</tr>
</table>

## light / ai.ai-settings.local

SSIM 0.6424

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.ai-settings.local.webp" width="480" alt="mock light ai.ai-settings.local"></td>
<td><img src="app-captures/light/ai.ai-settings.local.webp" width="480" alt="app light ai.ai-settings.local"></td>
</tr>
</table>

## dark / sales-invoice.item-list

SSIM 0.6427

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.item-list.webp" width="480" alt="mock dark sales-invoice.item-list"></td>
<td><img src="app-captures/dark/sales-invoice.item-list.webp" width="480" alt="app dark sales-invoice.item-list"></td>
</tr>
</table>

## hc / hi.sales-invoice.print-preview

SSIM 0.6428

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.sales-invoice.print-preview.webp" width="480" alt="mock hc hi.sales-invoice.print-preview"></td>
<td><img src="app-captures/hc/hi.sales-invoice.print-preview.webp" width="480" alt="app hc hi.sales-invoice.print-preview"></td>
</tr>
</table>

## light / ai.ai-settings.cloud

SSIM 0.6431

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.ai-settings.cloud.webp" width="480" alt="mock light ai.ai-settings.cloud"></td>
<td><img src="app-captures/light/ai.ai-settings.cloud.webp" width="480" alt="app light ai.ai-settings.cloud"></td>
</tr>
</table>

## dark / backup.backed-up

SSIM 0.6433

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/backup.backed-up.webp" width="480" alt="mock dark backup.backed-up"></td>
<td><img src="app-captures/dark/backup.backed-up.webp" width="480" alt="app dark backup.backed-up"></td>
</tr>
</table>

## hc / credit-note.validation-error

SSIM 0.6435

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/credit-note.validation-error.webp" width="480" alt="mock hc credit-note.validation-error"></td>
<td><img src="app-captures/hc/credit-note.validation-error.webp" width="480" alt="app hc credit-note.validation-error"></td>
</tr>
</table>

## light / hi.sales-invoice.default

SSIM 0.6448

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.sales-invoice.default.webp" width="480" alt="mock light hi.sales-invoice.default"></td>
<td><img src="app-captures/light/hi.sales-invoice.default.webp" width="480" alt="app light hi.sales-invoice.default"></td>
</tr>
</table>

## dark / ai.ai-settings.cloud

SSIM 0.6464

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.ai-settings.cloud.webp" width="480" alt="mock dark ai.ai-settings.cloud"></td>
<td><img src="app-captures/dark/ai.ai-settings.cloud.webp" width="480" alt="app dark ai.ai-settings.cloud"></td>
</tr>
</table>

## hc / debit-note.validation-error

SSIM 0.6469

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/debit-note.validation-error.webp" width="480" alt="mock hc debit-note.validation-error"></td>
<td><img src="app-captures/hc/debit-note.validation-error.webp" width="480" alt="app hc debit-note.validation-error"></td>
</tr>
</table>

## light / sales-invoice.mode-list

SSIM 0.6469

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.mode-list.webp" width="480" alt="mock light sales-invoice.mode-list"></td>
<td><img src="app-captures/light/sales-invoice.mode-list.webp" width="480" alt="app light sales-invoice.mode-list"></td>
</tr>
</table>

## dark / sales-invoice.as-voucher

SSIM 0.6474

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.as-voucher.webp" width="480" alt="mock dark sales-invoice.as-voucher"></td>
<td><img src="app-captures/dark/sales-invoice.as-voucher.webp" width="480" alt="app dark sales-invoice.as-voucher"></td>
</tr>
</table>

## dark / hi.import.file-missing-choice

SSIM 0.6480

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.import.file-missing-choice.webp" width="480" alt="mock dark hi.import.file-missing-choice"></td>
<td><img src="app-captures/dark/hi.import.file-missing-choice.webp" width="480" alt="app dark hi.import.file-missing-choice"></td>
</tr>
</table>

## light / profit-and-loss.provisional

SSIM 0.6486

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.provisional.webp" width="480" alt="mock light profit-and-loss.provisional"></td>
<td><img src="app-captures/light/profit-and-loss.provisional.webp" width="480" alt="app light profit-and-loss.provisional"></td>
</tr>
</table>

## dark / hi.sales-invoice.print-preview

SSIM 0.6489

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.sales-invoice.print-preview.webp" width="480" alt="mock dark hi.sales-invoice.print-preview"></td>
<td><img src="app-captures/dark/hi.sales-invoice.print-preview.webp" width="480" alt="app dark hi.sales-invoice.print-preview"></td>
</tr>
</table>

## dark / hi.masters.ledgers

SSIM 0.6497

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.masters.ledgers.webp" width="480" alt="mock dark hi.masters.ledgers"></td>
<td><img src="app-captures/dark/hi.masters.ledgers.webp" width="480" alt="app dark hi.masters.ledgers"></td>
</tr>
</table>

## dark / sales-invoice.mode-list

SSIM 0.6497

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.mode-list.webp" width="480" alt="mock dark sales-invoice.mode-list"></td>
<td><img src="app-captures/dark/sales-invoice.mode-list.webp" width="480" alt="app dark sales-invoice.mode-list"></td>
</tr>
</table>

## light / hi.bank.default

SSIM 0.6501

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.bank.default.webp" width="480" alt="mock light hi.bank.default"></td>
<td><img src="app-captures/light/hi.bank.default.webp" width="480" alt="app light hi.bank.default"></td>
</tr>
</table>

## dark / ai.ai-settings.local

SSIM 0.6516

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.ai-settings.local.webp" width="480" alt="mock dark ai.ai-settings.local"></td>
<td><img src="app-captures/dark/ai.ai-settings.local.webp" width="480" alt="app dark ai.ai-settings.local"></td>
</tr>
</table>

## dark / bank.brs-exported

SSIM 0.6520

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/bank.brs-exported.webp" width="480" alt="mock dark bank.brs-exported"></td>
<td><img src="app-captures/dark/bank.brs-exported.webp" width="480" alt="app dark bank.brs-exported"></td>
</tr>
</table>

## dark / balance-sheet.provisional

SSIM 0.6525

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/balance-sheet.provisional.webp" width="480" alt="mock dark balance-sheet.provisional"></td>
<td><img src="app-captures/dark/balance-sheet.provisional.webp" width="480" alt="app dark balance-sheet.provisional"></td>
</tr>
</table>

## hc / sales-invoice.b2c

SSIM 0.6528

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.b2c.webp" width="480" alt="mock hc sales-invoice.b2c"></td>
<td><img src="app-captures/hc/sales-invoice.b2c.webp" width="480" alt="app hc sales-invoice.b2c"></td>
</tr>
</table>

## dark / ai.ai-settings.local-download-failed

SSIM 0.6533

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.ai-settings.local-download-failed.webp" width="480" alt="mock dark ai.ai-settings.local-download-failed"></td>
<td><img src="app-captures/dark/ai.ai-settings.local-download-failed.webp" width="480" alt="app dark ai.ai-settings.local-download-failed"></td>
</tr>
</table>

## dark / import.default

SSIM 0.6537

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.default.webp" width="480" alt="mock dark import.default"></td>
<td><img src="app-captures/dark/import.default.webp" width="480" alt="app dark import.default"></td>
</tr>
</table>

## dark / profit-and-loss.detailed

SSIM 0.6538

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.detailed.webp" width="480" alt="mock dark profit-and-loss.detailed"></td>
<td><img src="app-captures/dark/profit-and-loss.detailed.webp" width="480" alt="app dark profit-and-loss.detailed"></td>
</tr>
</table>

## dark / sales-invoice.default

SSIM 0.6539

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.default.webp" width="480" alt="mock dark sales-invoice.default"></td>
<td><img src="app-captures/dark/sales-invoice.default.webp" width="480" alt="app dark sales-invoice.default"></td>
</tr>
</table>

## light / backup.backed-up

SSIM 0.6544

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/backup.backed-up.webp" width="480" alt="mock light backup.backed-up"></td>
<td><img src="app-captures/light/backup.backed-up.webp" width="480" alt="app light backup.backed-up"></td>
</tr>
</table>

## light / bank.brs-exported

SSIM 0.6544

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/bank.brs-exported.webp" width="480" alt="mock light bank.brs-exported"></td>
<td><img src="app-captures/light/bank.brs-exported.webp" width="480" alt="app light bank.brs-exported"></td>
</tr>
</table>

## light / ai.ask.result

SSIM 0.6559

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.ask.result.webp" width="480" alt="mock light ai.ask.result"></td>
<td><img src="app-captures/light/ai.ask.result.webp" width="480" alt="app light ai.ask.result"></td>
</tr>
</table>

## dark / trial-balance.default

SSIM 0.6561

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/trial-balance.default.webp" width="480" alt="mock dark trial-balance.default"></td>
<td><img src="app-captures/dark/trial-balance.default.webp" width="480" alt="app dark trial-balance.default"></td>
</tr>
</table>

## dark / ai.ask.result

SSIM 0.6563

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.ask.result.webp" width="480" alt="mock dark ai.ask.result"></td>
<td><img src="app-captures/dark/ai.ask.result.webp" width="480" alt="app dark ai.ask.result"></td>
</tr>
</table>

## dark / ai.ai-settings.cloud-edit

SSIM 0.6567

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.ai-settings.cloud-edit.webp" width="480" alt="mock dark ai.ai-settings.cloud-edit"></td>
<td><img src="app-captures/dark/ai.ai-settings.cloud-edit.webp" width="480" alt="app dark ai.ai-settings.cloud-edit"></td>
</tr>
</table>

## light / profit-and-loss.print-busy

SSIM 0.6569

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.print-busy.webp" width="480" alt="mock light profit-and-loss.print-busy"></td>
<td><img src="app-captures/light/profit-and-loss.print-busy.webp" width="480" alt="app light profit-and-loss.print-busy"></td>
</tr>
</table>

## dark / profit-and-loss.provisional

SSIM 0.6570

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.provisional.webp" width="480" alt="mock dark profit-and-loss.provisional"></td>
<td><img src="app-captures/dark/profit-and-loss.provisional.webp" width="480" alt="app dark profit-and-loss.provisional"></td>
</tr>
</table>

## dark / home.new-tab

SSIM 0.6571

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/home.new-tab.webp" width="480" alt="mock dark home.new-tab"></td>
<td><img src="app-captures/dark/home.new-tab.webp" width="480" alt="app dark home.new-tab"></td>
</tr>
</table>

## light / keys.standard

SSIM 0.6575

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/keys.standard.webp" width="480" alt="mock light keys.standard"></td>
<td><img src="app-captures/light/keys.standard.webp" width="480" alt="app light keys.standard"></td>
</tr>
</table>

## hc / palette.default

SSIM 0.6580

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/palette.default.webp" width="480" alt="mock hc palette.default"></td>
<td><img src="app-captures/hc/palette.default.webp" width="480" alt="app hc palette.default"></td>
</tr>
</table>

## dark / keys.standard

SSIM 0.6583

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/keys.standard.webp" width="480" alt="mock dark keys.standard"></td>
<td><img src="app-captures/dark/keys.standard.webp" width="480" alt="app dark keys.standard"></td>
</tr>
</table>

## hc / palette.voucher

SSIM 0.6586

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/palette.voucher.webp" width="480" alt="mock hc palette.voucher"></td>
<td><img src="app-captures/hc/palette.voucher.webp" width="480" alt="app hc palette.voucher"></td>
</tr>
</table>

## hc / purchase.default

SSIM 0.6588

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/purchase.default.webp" width="480" alt="mock hc purchase.default"></td>
<td><img src="app-captures/hc/purchase.default.webp" width="480" alt="app hc purchase.default"></td>
</tr>
</table>

## dark / sales-invoice.gst-inter-state

SSIM 0.6594

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.gst-inter-state.webp" width="480" alt="mock dark sales-invoice.gst-inter-state"></td>
<td><img src="app-captures/dark/sales-invoice.gst-inter-state.webp" width="480" alt="app dark sales-invoice.gst-inter-state"></td>
</tr>
</table>

## light / home.new-tab

SSIM 0.6596

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/home.new-tab.webp" width="480" alt="mock light home.new-tab"></td>
<td><img src="app-captures/light/home.new-tab.webp" width="480" alt="app light home.new-tab"></td>
</tr>
</table>

## dark / import.refused

SSIM 0.6596

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.refused.webp" width="480" alt="mock dark import.refused"></td>
<td><img src="app-captures/dark/import.refused.webp" width="480" alt="app dark import.refused"></td>
</tr>
</table>

## dark / import.port-error

SSIM 0.6602

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.port-error.webp" width="480" alt="mock dark import.port-error"></td>
<td><img src="app-captures/dark/import.port-error.webp" width="480" alt="app dark import.port-error"></td>
</tr>
</table>

## light / import.refused

SSIM 0.6603

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.refused.webp" width="480" alt="mock light import.refused"></td>
<td><img src="app-captures/light/import.refused.webp" width="480" alt="app light import.refused"></td>
</tr>
</table>

## light / import.default

SSIM 0.6604

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.default.webp" width="480" alt="mock light import.default"></td>
<td><img src="app-captures/light/import.default.webp" width="480" alt="app light import.default"></td>
</tr>
</table>

## dark / import.failed

SSIM 0.6605

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.failed.webp" width="480" alt="mock dark import.failed"></td>
<td><img src="app-captures/dark/import.failed.webp" width="480" alt="app dark import.failed"></td>
</tr>
</table>

## light / import.failed

SSIM 0.6606

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.failed.webp" width="480" alt="mock light import.failed"></td>
<td><img src="app-captures/light/import.failed.webp" width="480" alt="app light import.failed"></td>
</tr>
</table>

## light / balance-sheet.detailed

SSIM 0.6610

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/balance-sheet.detailed.webp" width="480" alt="mock light balance-sheet.detailed"></td>
<td><img src="app-captures/light/balance-sheet.detailed.webp" width="480" alt="app light balance-sheet.detailed"></td>
</tr>
</table>

## dark / import.validation-error

SSIM 0.6618

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.validation-error.webp" width="480" alt="mock dark import.validation-error"></td>
<td><img src="app-captures/dark/import.validation-error.webp" width="480" alt="app dark import.validation-error"></td>
</tr>
</table>

## light / profit-and-loss.print-failed

SSIM 0.6623

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.print-failed.webp" width="480" alt="mock light profit-and-loss.print-failed"></td>
<td><img src="app-captures/light/profit-and-loss.print-failed.webp" width="480" alt="app light profit-and-loss.print-failed"></td>
</tr>
</table>

## hc / palette.no-results

SSIM 0.6631

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/palette.no-results.webp" width="480" alt="mock hc palette.no-results"></td>
<td><img src="app-captures/hc/palette.no-results.webp" width="480" alt="app hc palette.no-results"></td>
</tr>
</table>

## dark / profit-and-loss.print-failed

SSIM 0.6632

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.print-failed.webp" width="480" alt="mock dark profit-and-loss.print-failed"></td>
<td><img src="app-captures/dark/profit-and-loss.print-failed.webp" width="480" alt="app dark profit-and-loss.print-failed"></td>
</tr>
</table>

## light / import.port-error

SSIM 0.6636

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.port-error.webp" width="480" alt="mock light import.port-error"></td>
<td><img src="app-captures/light/import.port-error.webp" width="480" alt="app light import.port-error"></td>
</tr>
</table>

## light / ai.ai-settings.cloud-edit

SSIM 0.6640

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.ai-settings.cloud-edit.webp" width="480" alt="mock light ai.ai-settings.cloud-edit"></td>
<td><img src="app-captures/light/ai.ai-settings.cloud-edit.webp" width="480" alt="app light ai.ai-settings.cloud-edit"></td>
</tr>
</table>

## dark / stock-summary.provisional

SSIM 0.6642

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/stock-summary.provisional.webp" width="480" alt="mock dark stock-summary.provisional"></td>
<td><img src="app-captures/dark/stock-summary.provisional.webp" width="480" alt="app dark stock-summary.provisional"></td>
</tr>
</table>

## dark / hi.sales-invoice.default

SSIM 0.6643

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.sales-invoice.default.webp" width="480" alt="mock dark hi.sales-invoice.default"></td>
<td><img src="app-captures/dark/hi.sales-invoice.default.webp" width="480" alt="app dark hi.sales-invoice.default"></td>
</tr>
</table>

## light / stock-summary.provisional

SSIM 0.6644

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/stock-summary.provisional.webp" width="480" alt="mock light stock-summary.provisional"></td>
<td><img src="app-captures/light/stock-summary.provisional.webp" width="480" alt="app light stock-summary.provisional"></td>
</tr>
</table>

## light / import.validation-error

SSIM 0.6650

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.validation-error.webp" width="480" alt="mock light import.validation-error"></td>
<td><img src="app-captures/light/import.validation-error.webp" width="480" alt="app light import.validation-error"></td>
</tr>
</table>

## hc / sales-invoice.saved

SSIM 0.6662

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/sales-invoice.saved.webp" width="480" alt="mock hc sales-invoice.saved"></td>
<td><img src="app-captures/hc/sales-invoice.saved.webp" width="480" alt="app hc sales-invoice.saved"></td>
</tr>
</table>

## light / profit-and-loss.print-range-error

SSIM 0.6665

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.print-range-error.webp" width="480" alt="mock light profit-and-loss.print-range-error"></td>
<td><img src="app-captures/light/profit-and-loss.print-range-error.webp" width="480" alt="app light profit-and-loss.print-range-error"></td>
</tr>
</table>

## hc / palette.search

SSIM 0.6665

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/palette.search.webp" width="480" alt="mock hc palette.search"></td>
<td><img src="app-captures/hc/palette.search.webp" width="480" alt="app hc palette.search"></td>
</tr>
</table>

## light / sales-invoice.b2c

SSIM 0.6666

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.b2c.webp" width="480" alt="mock light sales-invoice.b2c"></td>
<td><img src="app-captures/light/sales-invoice.b2c.webp" width="480" alt="app light sales-invoice.b2c"></td>
</tr>
</table>

## dark / home.ai-off

SSIM 0.6669

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/home.ai-off.webp" width="480" alt="mock dark home.ai-off"></td>
<td><img src="app-captures/dark/home.ai-off.webp" width="480" alt="app dark home.ai-off"></td>
</tr>
</table>

## dark / home.default

SSIM 0.6669

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/home.default.webp" width="480" alt="mock dark home.default"></td>
<td><img src="app-captures/dark/home.default.webp" width="480" alt="app dark home.default"></td>
</tr>
</table>

## hc / journal.print-preview

SSIM 0.6679

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/journal.print-preview.webp" width="480" alt="mock hc journal.print-preview"></td>
<td><img src="app-captures/hc/journal.print-preview.webp" width="480" alt="app hc journal.print-preview"></td>
</tr>
</table>

## hc / profit-and-loss.default

SSIM 0.6686

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.default.webp" width="480" alt="mock hc profit-and-loss.default"></td>
<td><img src="app-captures/hc/profit-and-loss.default.webp" width="480" alt="app hc profit-and-loss.default"></td>
</tr>
</table>

## light / balance-sheet.default

SSIM 0.6687

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/balance-sheet.default.webp" width="480" alt="mock light balance-sheet.default"></td>
<td><img src="app-captures/light/balance-sheet.default.webp" width="480" alt="app light balance-sheet.default"></td>
</tr>
</table>

## light / keys.classic

SSIM 0.6690

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/keys.classic.webp" width="480" alt="mock light keys.classic"></td>
<td><img src="app-captures/light/keys.classic.webp" width="480" alt="app light keys.classic"></td>
</tr>
</table>

## dark / keys.classic

SSIM 0.6692

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/keys.classic.webp" width="480" alt="mock dark keys.classic"></td>
<td><img src="app-captures/dark/keys.classic.webp" width="480" alt="app dark keys.classic"></td>
</tr>
</table>

## light / sales-invoice.print-preview

SSIM 0.6692

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.print-preview.webp" width="480" alt="mock light sales-invoice.print-preview"></td>
<td><img src="app-captures/light/sales-invoice.print-preview.webp" width="480" alt="app light sales-invoice.print-preview"></td>
</tr>
</table>

## hc / balance-sheet.voucher

SSIM 0.6710

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/balance-sheet.voucher.webp" width="480" alt="mock hc balance-sheet.voucher"></td>
<td><img src="app-captures/hc/balance-sheet.voucher.webp" width="480" alt="app hc balance-sheet.voucher"></td>
</tr>
</table>

## light / home.ai-off

SSIM 0.6712

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/home.ai-off.webp" width="480" alt="mock light home.ai-off"></td>
<td><img src="app-captures/light/home.ai-off.webp" width="480" alt="app light home.ai-off"></td>
</tr>
</table>

## light / home.default

SSIM 0.6712

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/home.default.webp" width="480" alt="mock light home.default"></td>
<td><img src="app-captures/light/home.default.webp" width="480" alt="app light home.default"></td>
</tr>
</table>

## dark / import.no-company

SSIM 0.6715

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.no-company.webp" width="480" alt="mock dark import.no-company"></td>
<td><img src="app-captures/dark/import.no-company.webp" width="480" alt="app dark import.no-company"></td>
</tr>
</table>

## hc / balance-sheet.ledger

SSIM 0.6716

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/balance-sheet.ledger.webp" width="480" alt="mock hc balance-sheet.ledger"></td>
<td><img src="app-captures/hc/balance-sheet.ledger.webp" width="480" alt="app hc balance-sheet.ledger"></td>
</tr>
</table>

## light / import.imported

SSIM 0.6717

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.imported.webp" width="480" alt="mock light import.imported"></td>
<td><img src="app-captures/light/import.imported.webp" width="480" alt="app light import.imported"></td>
</tr>
</table>

## dark / profit-and-loss.print-busy

SSIM 0.6723

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.print-busy.webp" width="480" alt="mock dark profit-and-loss.print-busy"></td>
<td><img src="app-captures/dark/profit-and-loss.print-busy.webp" width="480" alt="app dark profit-and-loss.print-busy"></td>
</tr>
</table>

## dark / import.imported

SSIM 0.6725

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.imported.webp" width="480" alt="mock dark import.imported"></td>
<td><img src="app-captures/dark/import.imported.webp" width="480" alt="app dark import.imported"></td>
</tr>
</table>

## light / hi.sales-invoice.print-preview

SSIM 0.6737

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.sales-invoice.print-preview.webp" width="480" alt="mock light hi.sales-invoice.print-preview"></td>
<td><img src="app-captures/light/hi.sales-invoice.print-preview.webp" width="480" alt="app light hi.sales-invoice.print-preview"></td>
</tr>
</table>

## dark / ai.purchase-bill.reading

SSIM 0.6747

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.purchase-bill.reading.webp" width="480" alt="mock dark ai.purchase-bill.reading"></td>
<td><img src="app-captures/dark/ai.purchase-bill.reading.webp" width="480" alt="app dark ai.purchase-bill.reading"></td>
</tr>
</table>

## dark / ai.quick-entry.unknown-ledger

SSIM 0.6750

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.quick-entry.unknown-ledger.webp" width="480" alt="mock dark ai.quick-entry.unknown-ledger"></td>
<td><img src="app-captures/dark/ai.quick-entry.unknown-ledger.webp" width="480" alt="app dark ai.quick-entry.unknown-ledger"></td>
</tr>
</table>

## dark / import.running

SSIM 0.6753

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/import.running.webp" width="480" alt="mock dark import.running"></td>
<td><img src="app-captures/dark/import.running.webp" width="480" alt="app dark import.running"></td>
</tr>
</table>

## light / bank.brs

SSIM 0.6755

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/bank.brs.webp" width="480" alt="mock light bank.brs"></td>
<td><img src="app-captures/light/bank.brs.webp" width="480" alt="app light bank.brs"></td>
</tr>
</table>

## light / ai.quick-entry.unknown-ledger

SSIM 0.6756

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.quick-entry.unknown-ledger.webp" width="480" alt="mock light ai.quick-entry.unknown-ledger"></td>
<td><img src="app-captures/light/ai.quick-entry.unknown-ledger.webp" width="480" alt="app light ai.quick-entry.unknown-ledger"></td>
</tr>
</table>

## light / profit-and-loss.print-config

SSIM 0.6756

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.print-config.webp" width="480" alt="mock light profit-and-loss.print-config"></td>
<td><img src="app-captures/light/profit-and-loss.print-config.webp" width="480" alt="app light profit-and-loss.print-config"></td>
</tr>
</table>

## hc / payment.overdraft

SSIM 0.6758

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/payment.overdraft.webp" width="480" alt="mock hc payment.overdraft"></td>
<td><img src="app-captures/hc/payment.overdraft.webp" width="480" alt="app hc payment.overdraft"></td>
</tr>
</table>

## light / import.running

SSIM 0.6769

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.running.webp" width="480" alt="mock light import.running"></td>
<td><img src="app-captures/light/import.running.webp" width="480" alt="app light import.running"></td>
</tr>
</table>

## light / profit-and-loss.print-preview

SSIM 0.6771

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.print-preview.webp" width="480" alt="mock light profit-and-loss.print-preview"></td>
<td><img src="app-captures/light/profit-and-loss.print-preview.webp" width="480" alt="app light profit-and-loss.print-preview"></td>
</tr>
</table>

## light / hi.balance-sheet.default

SSIM 0.6787

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.balance-sheet.default.webp" width="480" alt="mock light hi.balance-sheet.default"></td>
<td><img src="app-captures/light/hi.balance-sheet.default.webp" width="480" alt="app light hi.balance-sheet.default"></td>
</tr>
</table>

## light / gst.gstr1

SSIM 0.6792

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.gstr1.webp" width="480" alt="mock light gst.gstr1"></td>
<td><img src="app-captures/light/gst.gstr1.webp" width="480" alt="app light gst.gstr1"></td>
</tr>
</table>

## dark / bank.brs

SSIM 0.6797

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/bank.brs.webp" width="480" alt="mock dark bank.brs"></td>
<td><img src="app-captures/dark/bank.brs.webp" width="480" alt="app dark bank.brs"></td>
</tr>
</table>

## dark / gst.gstr1

SSIM 0.6834

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.gstr1.webp" width="480" alt="mock dark gst.gstr1"></td>
<td><img src="app-captures/dark/gst.gstr1.webp" width="480" alt="app dark gst.gstr1"></td>
</tr>
</table>

## light / import.no-company

SSIM 0.6834

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/import.no-company.webp" width="480" alt="mock light import.no-company"></td>
<td><img src="app-captures/light/import.no-company.webp" width="480" alt="app light import.no-company"></td>
</tr>
</table>

## dark / journal.print-preview

SSIM 0.6843

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/journal.print-preview.webp" width="480" alt="mock dark journal.print-preview"></td>
<td><img src="app-captures/dark/journal.print-preview.webp" width="480" alt="app dark journal.print-preview"></td>
</tr>
</table>

## light / ai-settings.off

SSIM 0.6865

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai-settings.off.webp" width="480" alt="mock light ai-settings.off"></td>
<td><img src="app-captures/light/ai-settings.off.webp" width="480" alt="app light ai-settings.off"></td>
</tr>
</table>

## light / sales-invoice.saved

SSIM 0.6867

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/sales-invoice.saved.webp" width="480" alt="mock light sales-invoice.saved"></td>
<td><img src="app-captures/light/sales-invoice.saved.webp" width="480" alt="app light sales-invoice.saved"></td>
</tr>
</table>

## dark / sales-invoice.b2c

SSIM 0.6867

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.b2c.webp" width="480" alt="mock dark sales-invoice.b2c"></td>
<td><img src="app-captures/dark/sales-invoice.b2c.webp" width="480" alt="app dark sales-invoice.b2c"></td>
</tr>
</table>

## hc / outstanding.receivables

SSIM 0.6886

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/outstanding.receivables.webp" width="480" alt="mock hc outstanding.receivables"></td>
<td><img src="app-captures/hc/outstanding.receivables.webp" width="480" alt="app hc outstanding.receivables"></td>
</tr>
</table>

## hc / journal.validation-error

SSIM 0.6891

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/journal.validation-error.webp" width="480" alt="mock hc journal.validation-error"></td>
<td><img src="app-captures/hc/journal.validation-error.webp" width="480" alt="app hc journal.validation-error"></td>
</tr>
</table>

## light / gst.gstr1-correction

SSIM 0.6901

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.gstr1-correction.webp" width="480" alt="mock light gst.gstr1-correction"></td>
<td><img src="app-captures/light/gst.gstr1-correction.webp" width="480" alt="app light gst.gstr1-correction"></td>
</tr>
</table>

## dark / ai-settings.off

SSIM 0.6906

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai-settings.off.webp" width="480" alt="mock dark ai-settings.off"></td>
<td><img src="app-captures/dark/ai-settings.off.webp" width="480" alt="app dark ai-settings.off"></td>
</tr>
</table>

## dark / backup.default

SSIM 0.6909

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/backup.default.webp" width="480" alt="mock dark backup.default"></td>
<td><img src="app-captures/dark/backup.default.webp" width="480" alt="app dark backup.default"></td>
</tr>
</table>

## dark / ai.quick-entry.accepted

SSIM 0.6928

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.quick-entry.accepted.webp" width="480" alt="mock dark ai.quick-entry.accepted"></td>
<td><img src="app-captures/dark/ai.quick-entry.accepted.webp" width="480" alt="app dark ai.quick-entry.accepted"></td>
</tr>
</table>

## light / hi.gst.gstr1

SSIM 0.6936

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.gst.gstr1.webp" width="480" alt="mock light hi.gst.gstr1"></td>
<td><img src="app-captures/light/hi.gst.gstr1.webp" width="480" alt="app light hi.gst.gstr1"></td>
</tr>
</table>

## dark / gst.gstr1-correction

SSIM 0.6946

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.gstr1-correction.webp" width="480" alt="mock dark gst.gstr1-correction"></td>
<td><img src="app-captures/dark/gst.gstr1-correction.webp" width="480" alt="app dark gst.gstr1-correction"></td>
</tr>
</table>

## hc / receipt.default

SSIM 0.6956

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/receipt.default.webp" width="480" alt="mock hc receipt.default"></td>
<td><img src="app-captures/hc/receipt.default.webp" width="480" alt="app hc receipt.default"></td>
</tr>
</table>

## light / ai.quick-entry.accepted

SSIM 0.6959

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.quick-entry.accepted.webp" width="480" alt="mock light ai.quick-entry.accepted"></td>
<td><img src="app-captures/light/ai.quick-entry.accepted.webp" width="480" alt="app light ai.quick-entry.accepted"></td>
</tr>
</table>

## hc / palette.commands

SSIM 0.6972

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/palette.commands.webp" width="480" alt="mock hc palette.commands"></td>
<td><img src="app-captures/hc/palette.commands.webp" width="480" alt="app hc palette.commands"></td>
</tr>
</table>

## dark / masters.no-match

SSIM 0.6979

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.no-match.webp" width="480" alt="mock dark masters.no-match"></td>
<td><img src="app-captures/dark/masters.no-match.webp" width="480" alt="app dark masters.no-match"></td>
</tr>
</table>

## light / hi.home.default

SSIM 0.6982

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.home.default.webp" width="480" alt="mock light hi.home.default"></td>
<td><img src="app-captures/light/hi.home.default.webp" width="480" alt="app light hi.home.default"></td>
</tr>
</table>

## hc / payment.bill-wise

SSIM 0.6985

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/payment.bill-wise.webp" width="480" alt="mock hc payment.bill-wise"></td>
<td><img src="app-captures/hc/payment.bill-wise.webp" width="480" alt="app hc payment.bill-wise"></td>
</tr>
</table>

## dark / hi.gst.gstr1

SSIM 0.6987

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.gst.gstr1.webp" width="480" alt="mock dark hi.gst.gstr1"></td>
<td><img src="app-captures/dark/hi.gst.gstr1.webp" width="480" alt="app dark hi.gst.gstr1"></td>
</tr>
</table>

## hc / credit-note.default

SSIM 0.6988

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/credit-note.default.webp" width="480" alt="mock hc credit-note.default"></td>
<td><img src="app-captures/hc/credit-note.default.webp" width="480" alt="app hc credit-note.default"></td>
</tr>
</table>

## light / outstanding.receivables

SSIM 0.6988

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/outstanding.receivables.webp" width="480" alt="mock light outstanding.receivables"></td>
<td><img src="app-captures/light/outstanding.receivables.webp" width="480" alt="app light outstanding.receivables"></td>
</tr>
</table>

## dark / balance-sheet.detailed

SSIM 0.7009

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/balance-sheet.detailed.webp" width="480" alt="mock dark balance-sheet.detailed"></td>
<td><img src="app-captures/dark/balance-sheet.detailed.webp" width="480" alt="app dark balance-sheet.detailed"></td>
</tr>
</table>

## dark / sales-invoice.saved

SSIM 0.7016

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/sales-invoice.saved.webp" width="480" alt="mock dark sales-invoice.saved"></td>
<td><img src="app-captures/dark/sales-invoice.saved.webp" width="480" alt="app dark sales-invoice.saved"></td>
</tr>
</table>

## dark / hi.home.default

SSIM 0.7025

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.home.default.webp" width="480" alt="mock dark hi.home.default"></td>
<td><img src="app-captures/dark/hi.home.default.webp" width="480" alt="app dark hi.home.default"></td>
</tr>
</table>

## light / purchase.validation-error

SSIM 0.7033

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/purchase.validation-error.webp" width="480" alt="mock light purchase.validation-error"></td>
<td><img src="app-captures/light/purchase.validation-error.webp" width="480" alt="app light purchase.validation-error"></td>
</tr>
</table>

## light / masters.no-match

SSIM 0.7041

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.no-match.webp" width="480" alt="mock light masters.no-match"></td>
<td><img src="app-captures/light/masters.no-match.webp" width="480" alt="app light masters.no-match"></td>
</tr>
</table>

## light / backup.default

SSIM 0.7059

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/backup.default.webp" width="480" alt="mock light backup.default"></td>
<td><img src="app-captures/light/backup.default.webp" width="480" alt="app light backup.default"></td>
</tr>
</table>

## dark / balance-sheet.default

SSIM 0.7063

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/balance-sheet.default.webp" width="480" alt="mock dark balance-sheet.default"></td>
<td><img src="app-captures/dark/balance-sheet.default.webp" width="480" alt="app dark balance-sheet.default"></td>
</tr>
</table>

## hc / debit-note.default

SSIM 0.7066

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/debit-note.default.webp" width="480" alt="mock hc debit-note.default"></td>
<td><img src="app-captures/hc/debit-note.default.webp" width="480" alt="app hc debit-note.default"></td>
</tr>
</table>

## light / profit-and-loss.default

SSIM 0.7073

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.default.webp" width="480" alt="mock light profit-and-loss.default"></td>
<td><img src="app-captures/light/profit-and-loss.default.webp" width="480" alt="app light profit-and-loss.default"></td>
</tr>
</table>

## dark / purchase.validation-error

SSIM 0.7075

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/purchase.validation-error.webp" width="480" alt="mock dark purchase.validation-error"></td>
<td><img src="app-captures/dark/purchase.validation-error.webp" width="480" alt="app dark purchase.validation-error"></td>
</tr>
</table>

## hc / ai.ask.unsupported

SSIM 0.7101

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.ask.unsupported.webp" width="480" alt="mock hc ai.ask.unsupported"></td>
<td><img src="app-captures/hc/ai.ask.unsupported.webp" width="480" alt="app hc ai.ask.unsupported"></td>
</tr>
</table>

## light / profit-and-loss.print-saved

SSIM 0.7102

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.print-saved.webp" width="480" alt="mock light profit-and-loss.print-saved"></td>
<td><img src="app-captures/light/profit-and-loss.print-saved.webp" width="480" alt="app light profit-and-loss.print-saved"></td>
</tr>
</table>

## dark / outstanding.receivables

SSIM 0.7112

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/outstanding.receivables.webp" width="480" alt="mock dark outstanding.receivables"></td>
<td><img src="app-captures/dark/outstanding.receivables.webp" width="480" alt="app dark outstanding.receivables"></td>
</tr>
</table>

## light / purchase.default

SSIM 0.7112

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/purchase.default.webp" width="480" alt="mock light purchase.default"></td>
<td><img src="app-captures/light/purchase.default.webp" width="480" alt="app light purchase.default"></td>
</tr>
</table>

## hc / setup.features

SSIM 0.7130

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.features.webp" width="480" alt="mock hc setup.features"></td>
<td><img src="app-captures/hc/setup.features.webp" width="480" alt="app hc setup.features"></td>
</tr>
</table>

## hc / balance-sheet.group

SSIM 0.7135

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/balance-sheet.group.webp" width="480" alt="mock hc balance-sheet.group"></td>
<td><img src="app-captures/hc/balance-sheet.group.webp" width="480" alt="app hc balance-sheet.group"></td>
</tr>
</table>

## hc / gst.recon

SSIM 0.7147

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.recon.webp" width="480" alt="mock hc gst.recon"></td>
<td><img src="app-captures/hc/gst.recon.webp" width="480" alt="app hc gst.recon"></td>
</tr>
</table>

## light / palette.default

SSIM 0.7156

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/palette.default.webp" width="480" alt="mock light palette.default"></td>
<td><img src="app-captures/light/palette.default.webp" width="480" alt="app light palette.default"></td>
</tr>
</table>

## hc / contra.default

SSIM 0.7158

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/contra.default.webp" width="480" alt="mock hc contra.default"></td>
<td><img src="app-captures/hc/contra.default.webp" width="480" alt="app hc contra.default"></td>
</tr>
</table>

## dark / hi.balance-sheet.default

SSIM 0.7163

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.balance-sheet.default.webp" width="480" alt="mock dark hi.balance-sheet.default"></td>
<td><img src="app-captures/dark/hi.balance-sheet.default.webp" width="480" alt="app dark hi.balance-sheet.default"></td>
</tr>
</table>

## hc / journal.default

SSIM 0.7165

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/journal.default.webp" width="480" alt="mock hc journal.default"></td>
<td><img src="app-captures/hc/journal.default.webp" width="480" alt="app hc journal.default"></td>
</tr>
</table>

## hc / stock-summary.default

SSIM 0.7171

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/stock-summary.default.webp" width="480" alt="mock hc stock-summary.default"></td>
<td><img src="app-captures/hc/stock-summary.default.webp" width="480" alt="app hc stock-summary.default"></td>
</tr>
</table>

## light / payment.overdraft

SSIM 0.7183

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/payment.overdraft.webp" width="480" alt="mock light payment.overdraft"></td>
<td><img src="app-captures/light/payment.overdraft.webp" width="480" alt="app light payment.overdraft"></td>
</tr>
</table>

## dark / profit-and-loss.print-saved

SSIM 0.7202

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.print-saved.webp" width="480" alt="mock dark profit-and-loss.print-saved"></td>
<td><img src="app-captures/dark/profit-and-loss.print-saved.webp" width="480" alt="app dark profit-and-loss.print-saved"></td>
</tr>
</table>

## dark / purchase.default

SSIM 0.7202

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/purchase.default.webp" width="480" alt="mock dark purchase.default"></td>
<td><img src="app-captures/dark/purchase.default.webp" width="480" alt="app dark purchase.default"></td>
</tr>
</table>

## light / palette.search

SSIM 0.7203

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/palette.search.webp" width="480" alt="mock light palette.search"></td>
<td><img src="app-captures/light/palette.search.webp" width="480" alt="app light palette.search"></td>
</tr>
</table>

## light / palette.voucher

SSIM 0.7213

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/palette.voucher.webp" width="480" alt="mock light palette.voucher"></td>
<td><img src="app-captures/light/palette.voucher.webp" width="480" alt="app light palette.voucher"></td>
</tr>
</table>

## light / day-book.print-preview

SSIM 0.7221

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/day-book.print-preview.webp" width="480" alt="mock light day-book.print-preview"></td>
<td><img src="app-captures/light/day-book.print-preview.webp" width="480" alt="app light day-book.print-preview"></td>
</tr>
</table>

## dark / masters.items

SSIM 0.7227

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.items.webp" width="480" alt="mock dark masters.items"></td>
<td><img src="app-captures/dark/masters.items.webp" width="480" alt="app dark masters.items"></td>
</tr>
</table>

## hc / ai.quick-entry.loading

SSIM 0.7227

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.quick-entry.loading.webp" width="480" alt="mock hc ai.quick-entry.loading"></td>
<td><img src="app-captures/hc/ai.quick-entry.loading.webp" width="480" alt="app hc ai.quick-entry.loading"></td>
</tr>
</table>

## dark / palette.default

SSIM 0.7232

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/palette.default.webp" width="480" alt="mock dark palette.default"></td>
<td><img src="app-captures/dark/palette.default.webp" width="480" alt="app dark palette.default"></td>
</tr>
</table>

## light / masters.loading

SSIM 0.7234

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.loading.webp" width="480" alt="mock light masters.loading"></td>
<td><img src="app-captures/light/masters.loading.webp" width="480" alt="app light masters.loading"></td>
</tr>
</table>

## hc / payment.default

SSIM 0.7240

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/payment.default.webp" width="480" alt="mock hc payment.default"></td>
<td><img src="app-captures/hc/payment.default.webp" width="480" alt="app hc payment.default"></td>
</tr>
</table>

## dark / profit-and-loss.default

SSIM 0.7241

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.default.webp" width="480" alt="mock dark profit-and-loss.default"></td>
<td><img src="app-captures/dark/profit-and-loss.default.webp" width="480" alt="app dark profit-and-loss.default"></td>
</tr>
</table>

## light / palette.no-results

SSIM 0.7247

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/palette.no-results.webp" width="480" alt="mock light palette.no-results"></td>
<td><img src="app-captures/light/palette.no-results.webp" width="480" alt="app light palette.no-results"></td>
</tr>
</table>

## dark / payment.overdraft

SSIM 0.7247

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/payment.overdraft.webp" width="480" alt="mock dark payment.overdraft"></td>
<td><img src="app-captures/dark/payment.overdraft.webp" width="480" alt="app dark payment.overdraft"></td>
</tr>
</table>

## light / masters.items

SSIM 0.7250

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.items.webp" width="480" alt="mock light masters.items"></td>
<td><img src="app-captures/light/masters.items.webp" width="480" alt="app light masters.items"></td>
</tr>
</table>

## hc / ai.quick-entry.not-installed

SSIM 0.7251

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.quick-entry.not-installed.webp" width="480" alt="mock hc ai.quick-entry.not-installed"></td>
<td><img src="app-captures/hc/ai.quick-entry.not-installed.webp" width="480" alt="app hc ai.quick-entry.not-installed"></td>
</tr>
</table>

## light / credit-note.validation-error

SSIM 0.7262

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/credit-note.validation-error.webp" width="480" alt="mock light credit-note.validation-error"></td>
<td><img src="app-captures/light/credit-note.validation-error.webp" width="480" alt="app light credit-note.validation-error"></td>
</tr>
</table>

## light / debit-note.validation-error

SSIM 0.7275

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/debit-note.validation-error.webp" width="480" alt="mock light debit-note.validation-error"></td>
<td><img src="app-captures/light/debit-note.validation-error.webp" width="480" alt="app light debit-note.validation-error"></td>
</tr>
</table>

## hc / setup.new-tab

SSIM 0.7278

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.new-tab.webp" width="480" alt="mock hc setup.new-tab"></td>
<td><img src="app-captures/hc/setup.new-tab.webp" width="480" alt="app hc setup.new-tab"></td>
</tr>
</table>

## hc / home.loading

SSIM 0.7280

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/home.loading.webp" width="480" alt="mock hc home.loading"></td>
<td><img src="app-captures/hc/home.loading.webp" width="480" alt="app hc home.loading"></td>
</tr>
</table>

## light / balance-sheet.voucher

SSIM 0.7284

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/balance-sheet.voucher.webp" width="480" alt="mock light balance-sheet.voucher"></td>
<td><img src="app-captures/light/balance-sheet.voucher.webp" width="480" alt="app light balance-sheet.voucher"></td>
</tr>
</table>

## hc / payment.account-list

SSIM 0.7297

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/payment.account-list.webp" width="480" alt="mock hc payment.account-list"></td>
<td><img src="app-captures/hc/payment.account-list.webp" width="480" alt="app hc payment.account-list"></td>
</tr>
</table>

## light / receipt.default

SSIM 0.7312

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/receipt.default.webp" width="480" alt="mock light receipt.default"></td>
<td><img src="app-captures/light/receipt.default.webp" width="480" alt="app light receipt.default"></td>
</tr>
</table>

## dark / masters.loading

SSIM 0.7313

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.loading.webp" width="480" alt="mock dark masters.loading"></td>
<td><img src="app-captures/dark/masters.loading.webp" width="480" alt="app dark masters.loading"></td>
</tr>
</table>

## light / stock-summary.default

SSIM 0.7329

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/stock-summary.default.webp" width="480" alt="mock light stock-summary.default"></td>
<td><img src="app-captures/light/stock-summary.default.webp" width="480" alt="app light stock-summary.default"></td>
</tr>
</table>

## dark / credit-note.validation-error

SSIM 0.7330

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/credit-note.validation-error.webp" width="480" alt="mock dark credit-note.validation-error"></td>
<td><img src="app-captures/dark/credit-note.validation-error.webp" width="480" alt="app dark credit-note.validation-error"></td>
</tr>
</table>

## light / payment.bill-wise

SSIM 0.7335

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/payment.bill-wise.webp" width="480" alt="mock light payment.bill-wise"></td>
<td><img src="app-captures/light/payment.bill-wise.webp" width="480" alt="app light payment.bill-wise"></td>
</tr>
</table>

## hc / ai.ask.failed

SSIM 0.7338

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.ask.failed.webp" width="480" alt="mock hc ai.ask.failed"></td>
<td><img src="app-captures/hc/ai.ask.failed.webp" width="480" alt="app hc ai.ask.failed"></td>
</tr>
</table>

## light / balance-sheet.group

SSIM 0.7339

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/balance-sheet.group.webp" width="480" alt="mock light balance-sheet.group"></td>
<td><img src="app-captures/light/balance-sheet.group.webp" width="480" alt="app light balance-sheet.group"></td>
</tr>
</table>

## dark / debit-note.validation-error

SSIM 0.7345

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/debit-note.validation-error.webp" width="480" alt="mock dark debit-note.validation-error"></td>
<td><img src="app-captures/dark/debit-note.validation-error.webp" width="480" alt="app dark debit-note.validation-error"></td>
</tr>
</table>

## dark / palette.search

SSIM 0.7347

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/palette.search.webp" width="480" alt="mock dark palette.search"></td>
<td><img src="app-captures/dark/palette.search.webp" width="480" alt="app dark palette.search"></td>
</tr>
</table>

## light / gst.recon

SSIM 0.7376

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.recon.webp" width="480" alt="mock light gst.recon"></td>
<td><img src="app-captures/light/gst.recon.webp" width="480" alt="app light gst.recon"></td>
</tr>
</table>

## dark / balance-sheet.voucher

SSIM 0.7380

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/balance-sheet.voucher.webp" width="480" alt="mock dark balance-sheet.voucher"></td>
<td><img src="app-captures/dark/balance-sheet.voucher.webp" width="480" alt="app dark balance-sheet.voucher"></td>
</tr>
</table>

## hc / masters.ledger-detail

SSIM 0.7380

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.ledger-detail.webp" width="480" alt="mock hc masters.ledger-detail"></td>
<td><img src="app-captures/hc/masters.ledger-detail.webp" width="480" alt="app hc masters.ledger-detail"></td>
</tr>
</table>

## light / palette.commands

SSIM 0.7380

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/palette.commands.webp" width="480" alt="mock light palette.commands"></td>
<td><img src="app-captures/light/palette.commands.webp" width="480" alt="app light palette.commands"></td>
</tr>
</table>

## dark / palette.no-results

SSIM 0.7385

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/palette.no-results.webp" width="480" alt="mock dark palette.no-results"></td>
<td><img src="app-captures/dark/palette.no-results.webp" width="480" alt="app dark palette.no-results"></td>
</tr>
</table>

## hc / contra.validation-error

SSIM 0.7389

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/contra.validation-error.webp" width="480" alt="mock hc contra.validation-error"></td>
<td><img src="app-captures/hc/contra.validation-error.webp" width="480" alt="app hc contra.validation-error"></td>
</tr>
</table>

## dark / palette.voucher

SSIM 0.7395

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/palette.voucher.webp" width="480" alt="mock dark palette.voucher"></td>
<td><img src="app-captures/dark/palette.voucher.webp" width="480" alt="app dark palette.voucher"></td>
</tr>
</table>

## dark / receipt.default

SSIM 0.7403

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/receipt.default.webp" width="480" alt="mock dark receipt.default"></td>
<td><img src="app-captures/dark/receipt.default.webp" width="480" alt="app dark receipt.default"></td>
</tr>
</table>

## dark / balance-sheet.ledger

SSIM 0.7416

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/balance-sheet.ledger.webp" width="480" alt="mock dark balance-sheet.ledger"></td>
<td><img src="app-captures/dark/balance-sheet.ledger.webp" width="480" alt="app dark balance-sheet.ledger"></td>
</tr>
</table>

## light / balance-sheet.ledger

SSIM 0.7425

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/balance-sheet.ledger.webp" width="480" alt="mock light balance-sheet.ledger"></td>
<td><img src="app-captures/light/balance-sheet.ledger.webp" width="480" alt="app light balance-sheet.ledger"></td>
</tr>
</table>

## dark / balance-sheet.group

SSIM 0.7429

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/balance-sheet.group.webp" width="480" alt="mock dark balance-sheet.group"></td>
<td><img src="app-captures/dark/balance-sheet.group.webp" width="480" alt="app dark balance-sheet.group"></td>
</tr>
</table>

## dark / payment.bill-wise

SSIM 0.7430

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/payment.bill-wise.webp" width="480" alt="mock dark payment.bill-wise"></td>
<td><img src="app-captures/dark/payment.bill-wise.webp" width="480" alt="app dark payment.bill-wise"></td>
</tr>
</table>

## dark / gst.recon

SSIM 0.7437

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.recon.webp" width="480" alt="mock dark gst.recon"></td>
<td><img src="app-captures/dark/gst.recon.webp" width="480" alt="app dark gst.recon"></td>
</tr>
</table>

## hc / day-book.default

SSIM 0.7441

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/day-book.default.webp" width="480" alt="mock hc day-book.default"></td>
<td><img src="app-captures/hc/day-book.default.webp" width="480" alt="app hc day-book.default"></td>
</tr>
</table>

## light / outstanding.payables

SSIM 0.7444

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/outstanding.payables.webp" width="480" alt="mock light outstanding.payables"></td>
<td><img src="app-captures/light/outstanding.payables.webp" width="480" alt="app light outstanding.payables"></td>
</tr>
</table>

## hc / ai.quick-entry.read-failed

SSIM 0.7452

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.quick-entry.read-failed.webp" width="480" alt="mock hc ai.quick-entry.read-failed"></td>
<td><img src="app-captures/hc/ai.quick-entry.read-failed.webp" width="480" alt="app hc ai.quick-entry.read-failed"></td>
</tr>
</table>

## dark / stock-summary.default

SSIM 0.7454

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/stock-summary.default.webp" width="480" alt="mock dark stock-summary.default"></td>
<td><img src="app-captures/dark/stock-summary.default.webp" width="480" alt="app dark stock-summary.default"></td>
</tr>
</table>

## hc / outstanding.payables

SSIM 0.7474

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/outstanding.payables.webp" width="480" alt="mock hc outstanding.payables"></td>
<td><img src="app-captures/hc/outstanding.payables.webp" width="480" alt="app hc outstanding.payables"></td>
</tr>
</table>

## dark / palette.commands

SSIM 0.7500

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/palette.commands.webp" width="480" alt="mock dark palette.commands"></td>
<td><img src="app-captures/dark/palette.commands.webp" width="480" alt="app dark palette.commands"></td>
</tr>
</table>

## dark / outstanding.payables

SSIM 0.7526

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/outstanding.payables.webp" width="480" alt="mock dark outstanding.payables"></td>
<td><img src="app-captures/dark/outstanding.payables.webp" width="480" alt="app dark outstanding.payables"></td>
</tr>
</table>

## light / payment.default

SSIM 0.7528

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/payment.default.webp" width="480" alt="mock light payment.default"></td>
<td><img src="app-captures/light/payment.default.webp" width="480" alt="app light payment.default"></td>
</tr>
</table>

## hc / bank.import-error

SSIM 0.7539

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/bank.import-error.webp" width="480" alt="mock hc bank.import-error"></td>
<td><img src="app-captures/hc/bank.import-error.webp" width="480" alt="app hc bank.import-error"></td>
</tr>
</table>

## hc / profit-and-loss.loading

SSIM 0.7562

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/profit-and-loss.loading.webp" width="480" alt="mock hc profit-and-loss.loading"></td>
<td><img src="app-captures/hc/profit-and-loss.loading.webp" width="480" alt="app hc profit-and-loss.loading"></td>
</tr>
</table>

## hc / ai.ask.idle

SSIM 0.7571

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.ask.idle.webp" width="480" alt="mock hc ai.ask.idle"></td>
<td><img src="app-captures/hc/ai.ask.idle.webp" width="480" alt="app hc ai.ask.idle"></td>
</tr>
</table>

## hc / masters.create-ledger

SSIM 0.7578

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.create-ledger.webp" width="480" alt="mock hc masters.create-ledger"></td>
<td><img src="app-captures/hc/masters.create-ledger.webp" width="480" alt="app hc masters.create-ledger"></td>
</tr>
</table>

## light / credit-note.default

SSIM 0.7585

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/credit-note.default.webp" width="480" alt="mock light credit-note.default"></td>
<td><img src="app-captures/light/credit-note.default.webp" width="480" alt="app light credit-note.default"></td>
</tr>
</table>

## hc / ai.purchase-bill.rejected

SSIM 0.7587

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.purchase-bill.rejected.webp" width="480" alt="mock hc ai.purchase-bill.rejected"></td>
<td><img src="app-captures/hc/ai.purchase-bill.rejected.webp" width="480" alt="app hc ai.purchase-bill.rejected"></td>
</tr>
</table>

## dark / payment.default

SSIM 0.7602

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/payment.default.webp" width="480" alt="mock dark payment.default"></td>
<td><img src="app-captures/dark/payment.default.webp" width="480" alt="app dark payment.default"></td>
</tr>
</table>

## light / ai.quick-entry.loading

SSIM 0.7610

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.quick-entry.loading.webp" width="480" alt="mock light ai.quick-entry.loading"></td>
<td><img src="app-captures/light/ai.quick-entry.loading.webp" width="480" alt="app light ai.quick-entry.loading"></td>
</tr>
</table>

## light / day-book.default

SSIM 0.7621

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/day-book.default.webp" width="480" alt="mock light day-book.default"></td>
<td><img src="app-captures/light/day-book.default.webp" width="480" alt="app light day-book.default"></td>
</tr>
</table>

## light / debit-note.default

SSIM 0.7626

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/debit-note.default.webp" width="480" alt="mock light debit-note.default"></td>
<td><img src="app-captures/light/debit-note.default.webp" width="480" alt="app light debit-note.default"></td>
</tr>
</table>

## light / ai.ask.unsupported

SSIM 0.7636

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.ask.unsupported.webp" width="480" alt="mock light ai.ask.unsupported"></td>
<td><img src="app-captures/light/ai.ask.unsupported.webp" width="480" alt="app light ai.ask.unsupported"></td>
</tr>
</table>

## dark / credit-note.default

SSIM 0.7642

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/credit-note.default.webp" width="480" alt="mock dark credit-note.default"></td>
<td><img src="app-captures/dark/credit-note.default.webp" width="480" alt="app dark credit-note.default"></td>
</tr>
</table>

## light / setup.features

SSIM 0.7643

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.features.webp" width="480" alt="mock light setup.features"></td>
<td><img src="app-captures/light/setup.features.webp" width="480" alt="app light setup.features"></td>
</tr>
</table>

## hc / home.empty

SSIM 0.7647

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/home.empty.webp" width="480" alt="mock hc home.empty"></td>
<td><img src="app-captures/hc/home.empty.webp" width="480" alt="app hc home.empty"></td>
</tr>
</table>

## hc / audit.integrity

SSIM 0.7658

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/audit.integrity.webp" width="480" alt="mock hc audit.integrity"></td>
<td><img src="app-captures/hc/audit.integrity.webp" width="480" alt="app hc audit.integrity"></td>
</tr>
</table>

## light / payment.account-list

SSIM 0.7662

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/payment.account-list.webp" width="480" alt="mock light payment.account-list"></td>
<td><img src="app-captures/light/payment.account-list.webp" width="480" alt="app light payment.account-list"></td>
</tr>
</table>

## hc / balance-sheet.loading

SSIM 0.7664

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/balance-sheet.loading.webp" width="480" alt="mock hc balance-sheet.loading"></td>
<td><img src="app-captures/hc/balance-sheet.loading.webp" width="480" alt="app hc balance-sheet.loading"></td>
</tr>
</table>

## hc / masters.gstin-error

SSIM 0.7676

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.gstin-error.webp" width="480" alt="mock hc masters.gstin-error"></td>
<td><img src="app-captures/hc/masters.gstin-error.webp" width="480" alt="app hc masters.gstin-error"></td>
</tr>
</table>

## hc / ai.quick-entry.idle

SSIM 0.7678

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.quick-entry.idle.webp" width="480" alt="mock hc ai.quick-entry.idle"></td>
<td><img src="app-captures/hc/ai.quick-entry.idle.webp" width="480" alt="app hc ai.quick-entry.idle"></td>
</tr>
</table>

## dark / ai.ask.unsupported

SSIM 0.7691

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.ask.unsupported.webp" width="480" alt="mock dark ai.ask.unsupported"></td>
<td><img src="app-captures/dark/ai.ask.unsupported.webp" width="480" alt="app dark ai.ask.unsupported"></td>
</tr>
</table>

## dark / debit-note.default

SSIM 0.7692

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/debit-note.default.webp" width="480" alt="mock dark debit-note.default"></td>
<td><img src="app-captures/dark/debit-note.default.webp" width="480" alt="app dark debit-note.default"></td>
</tr>
</table>

## dark / ai.purchase-bill.rejected

SSIM 0.7694

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.purchase-bill.rejected.webp" width="480" alt="mock dark ai.purchase-bill.rejected"></td>
<td><img src="app-captures/dark/ai.purchase-bill.rejected.webp" width="480" alt="app dark ai.purchase-bill.rejected"></td>
</tr>
</table>

## light / journal.validation-error

SSIM 0.7695

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/journal.validation-error.webp" width="480" alt="mock light journal.validation-error"></td>
<td><img src="app-captures/light/journal.validation-error.webp" width="480" alt="app light journal.validation-error"></td>
</tr>
</table>

## light / ai.purchase-bill.rejected

SSIM 0.7696

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.purchase-bill.rejected.webp" width="480" alt="mock light ai.purchase-bill.rejected"></td>
<td><img src="app-captures/light/ai.purchase-bill.rejected.webp" width="480" alt="app light ai.purchase-bill.rejected"></td>
</tr>
</table>

## light / contra.default

SSIM 0.7699

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/contra.default.webp" width="480" alt="mock light contra.default"></td>
<td><img src="app-captures/light/contra.default.webp" width="480" alt="app light contra.default"></td>
</tr>
</table>

## dark / masters.ledger-detail

SSIM 0.7703

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.ledger-detail.webp" width="480" alt="mock dark masters.ledger-detail"></td>
<td><img src="app-captures/dark/masters.ledger-detail.webp" width="480" alt="app dark masters.ledger-detail"></td>
</tr>
</table>

## dark / ai.quick-entry.loading

SSIM 0.7712

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.quick-entry.loading.webp" width="480" alt="mock dark ai.quick-entry.loading"></td>
<td><img src="app-captures/dark/ai.quick-entry.loading.webp" width="480" alt="app dark ai.quick-entry.loading"></td>
</tr>
</table>

## dark / day-book.default

SSIM 0.7717

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/day-book.default.webp" width="480" alt="mock dark day-book.default"></td>
<td><img src="app-captures/dark/day-book.default.webp" width="480" alt="app dark day-book.default"></td>
</tr>
</table>

## light / ai.purchase-bill.reading

SSIM 0.7718

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.purchase-bill.reading.webp" width="480" alt="mock light ai.purchase-bill.reading"></td>
<td><img src="app-captures/light/ai.purchase-bill.reading.webp" width="480" alt="app light ai.purchase-bill.reading"></td>
</tr>
</table>

## light / setup.new-tab

SSIM 0.7721

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.new-tab.webp" width="480" alt="mock light setup.new-tab"></td>
<td><img src="app-captures/light/setup.new-tab.webp" width="480" alt="app light setup.new-tab"></td>
</tr>
</table>

## dark / setup.features

SSIM 0.7724

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.features.webp" width="480" alt="mock dark setup.features"></td>
<td><img src="app-captures/dark/setup.features.webp" width="480" alt="app dark setup.features"></td>
</tr>
</table>

## hc / stock-summary.loading

SSIM 0.7726

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/stock-summary.loading.webp" width="480" alt="mock hc stock-summary.loading"></td>
<td><img src="app-captures/hc/stock-summary.loading.webp" width="480" alt="app hc stock-summary.loading"></td>
</tr>
</table>

## light / ai.quick-entry.not-installed

SSIM 0.7726

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.quick-entry.not-installed.webp" width="480" alt="mock light ai.quick-entry.not-installed"></td>
<td><img src="app-captures/light/ai.quick-entry.not-installed.webp" width="480" alt="app light ai.quick-entry.not-installed"></td>
</tr>
</table>

## dark / journal.validation-error

SSIM 0.7729

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/journal.validation-error.webp" width="480" alt="mock dark journal.validation-error"></td>
<td><img src="app-captures/dark/journal.validation-error.webp" width="480" alt="app dark journal.validation-error"></td>
</tr>
</table>

## dark / ai.quick-entry.not-installed

SSIM 0.7731

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.quick-entry.not-installed.webp" width="480" alt="mock dark ai.quick-entry.not-installed"></td>
<td><img src="app-captures/dark/ai.quick-entry.not-installed.webp" width="480" alt="app dark ai.quick-entry.not-installed"></td>
</tr>
</table>

## hc / audit.loading

SSIM 0.7733

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/audit.loading.webp" width="480" alt="mock hc audit.loading"></td>
<td><img src="app-captures/hc/audit.loading.webp" width="480" alt="app hc audit.loading"></td>
</tr>
</table>

## hc / outstanding.loading

SSIM 0.7734

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/outstanding.loading.webp" width="480" alt="mock hc outstanding.loading"></td>
<td><img src="app-captures/hc/outstanding.loading.webp" width="480" alt="app hc outstanding.loading"></td>
</tr>
</table>

## dark / payment.account-list

SSIM 0.7735

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/payment.account-list.webp" width="480" alt="mock dark payment.account-list"></td>
<td><img src="app-captures/dark/payment.account-list.webp" width="480" alt="app dark payment.account-list"></td>
</tr>
</table>

## hc / trial-balance.loading

SSIM 0.7741

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/trial-balance.loading.webp" width="480" alt="mock hc trial-balance.loading"></td>
<td><img src="app-captures/hc/trial-balance.loading.webp" width="480" alt="app hc trial-balance.loading"></td>
</tr>
</table>

## dark / contra.default

SSIM 0.7755

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/contra.default.webp" width="480" alt="mock dark contra.default"></td>
<td><img src="app-captures/dark/contra.default.webp" width="480" alt="app dark contra.default"></td>
</tr>
</table>

## dark / setup.new-tab

SSIM 0.7764

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.new-tab.webp" width="480" alt="mock dark setup.new-tab"></td>
<td><img src="app-captures/dark/setup.new-tab.webp" width="480" alt="app dark setup.new-tab"></td>
</tr>
</table>

## light / masters.ledger-detail

SSIM 0.7781

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.ledger-detail.webp" width="480" alt="mock light masters.ledger-detail"></td>
<td><img src="app-captures/light/masters.ledger-detail.webp" width="480" alt="app light masters.ledger-detail"></td>
</tr>
</table>

## light / ai.ask.failed

SSIM 0.7787

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.ask.failed.webp" width="480" alt="mock light ai.ask.failed"></td>
<td><img src="app-captures/light/ai.ask.failed.webp" width="480" alt="app light ai.ask.failed"></td>
</tr>
</table>

## hc / setup.company

SSIM 0.7806

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.company.webp" width="480" alt="mock hc setup.company"></td>
<td><img src="app-captures/hc/setup.company.webp" width="480" alt="app hc setup.company"></td>
</tr>
</table>

## light / journal.print-preview

SSIM 0.7810

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/journal.print-preview.webp" width="480" alt="mock light journal.print-preview"></td>
<td><img src="app-captures/light/journal.print-preview.webp" width="480" alt="app light journal.print-preview"></td>
</tr>
</table>

## dark / ai.ask.failed

SSIM 0.7821

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.ask.failed.webp" width="480" alt="mock dark ai.ask.failed"></td>
<td><img src="app-captures/dark/ai.ask.failed.webp" width="480" alt="app dark ai.ask.failed"></td>
</tr>
</table>

## light / journal.default

SSIM 0.7835

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/journal.default.webp" width="480" alt="mock light journal.default"></td>
<td><img src="app-captures/light/journal.default.webp" width="480" alt="app light journal.default"></td>
</tr>
</table>

## dark / ai.quick-entry.read-failed

SSIM 0.7853

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.quick-entry.read-failed.webp" width="480" alt="mock dark ai.quick-entry.read-failed"></td>
<td><img src="app-captures/dark/ai.quick-entry.read-failed.webp" width="480" alt="app dark ai.quick-entry.read-failed"></td>
</tr>
</table>

## light / contra.validation-error

SSIM 0.7868

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/contra.validation-error.webp" width="480" alt="mock light contra.validation-error"></td>
<td><img src="app-captures/light/contra.validation-error.webp" width="480" alt="app light contra.validation-error"></td>
</tr>
</table>

## light / ai.quick-entry.read-failed

SSIM 0.7873

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.quick-entry.read-failed.webp" width="480" alt="mock light ai.quick-entry.read-failed"></td>
<td><img src="app-captures/light/ai.quick-entry.read-failed.webp" width="480" alt="app light ai.quick-entry.read-failed"></td>
</tr>
</table>

## dark / journal.default

SSIM 0.7876

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/journal.default.webp" width="480" alt="mock dark journal.default"></td>
<td><img src="app-captures/dark/journal.default.webp" width="480" alt="app dark journal.default"></td>
</tr>
</table>

## light / ai.ask.idle

SSIM 0.7880

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.ask.idle.webp" width="480" alt="mock light ai.ask.idle"></td>
<td><img src="app-captures/light/ai.ask.idle.webp" width="480" alt="app light ai.ask.idle"></td>
</tr>
</table>

## hc / bank.empty

SSIM 0.7898

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/bank.empty.webp" width="480" alt="mock hc bank.empty"></td>
<td><img src="app-captures/hc/bank.empty.webp" width="480" alt="app hc bank.empty"></td>
</tr>
</table>

## dark / ai.ask.idle

SSIM 0.7899

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.ask.idle.webp" width="480" alt="mock dark ai.ask.idle"></td>
<td><img src="app-captures/dark/ai.ask.idle.webp" width="480" alt="app dark ai.ask.idle"></td>
</tr>
</table>

## hc / masters.create-item

SSIM 0.7902

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.create-item.webp" width="480" alt="mock hc masters.create-item"></td>
<td><img src="app-captures/hc/masters.create-item.webp" width="480" alt="app hc masters.create-item"></td>
</tr>
</table>

## dark / contra.validation-error

SSIM 0.7934

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/contra.validation-error.webp" width="480" alt="mock dark contra.validation-error"></td>
<td><img src="app-captures/dark/contra.validation-error.webp" width="480" alt="app dark contra.validation-error"></td>
</tr>
</table>

## hc / gst.recon-locked

SSIM 0.7962

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.recon-locked.webp" width="480" alt="mock hc gst.recon-locked"></td>
<td><img src="app-captures/hc/gst.recon-locked.webp" width="480" alt="app hc gst.recon-locked"></td>
</tr>
</table>

## hc / masters.items-empty

SSIM 0.7986

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.items-empty.webp" width="480" alt="mock hc masters.items-empty"></td>
<td><img src="app-captures/hc/masters.items-empty.webp" width="480" alt="app hc masters.items-empty"></td>
</tr>
</table>

## dark / setup.company

SSIM 0.7995

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.company.webp" width="480" alt="mock dark setup.company"></td>
<td><img src="app-captures/dark/setup.company.webp" width="480" alt="app dark setup.company"></td>
</tr>
</table>

## hc / masters.empty

SSIM 0.7996

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/masters.empty.webp" width="480" alt="mock hc masters.empty"></td>
<td><img src="app-captures/hc/masters.empty.webp" width="480" alt="app hc masters.empty"></td>
</tr>
</table>

## light / masters.gstin-error

SSIM 0.8010

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.gstin-error.webp" width="480" alt="mock light masters.gstin-error"></td>
<td><img src="app-captures/light/masters.gstin-error.webp" width="480" alt="app light masters.gstin-error"></td>
</tr>
</table>

## light / ai.quick-entry.idle

SSIM 0.8015

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.quick-entry.idle.webp" width="480" alt="mock light ai.quick-entry.idle"></td>
<td><img src="app-captures/light/ai.quick-entry.idle.webp" width="480" alt="app light ai.quick-entry.idle"></td>
</tr>
</table>

## dark / masters.gstin-error

SSIM 0.8018

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.gstin-error.webp" width="480" alt="mock dark masters.gstin-error"></td>
<td><img src="app-captures/dark/masters.gstin-error.webp" width="480" alt="app dark masters.gstin-error"></td>
</tr>
</table>

## light / masters.create-ledger

SSIM 0.8020

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.create-ledger.webp" width="480" alt="mock light masters.create-ledger"></td>
<td><img src="app-captures/light/masters.create-ledger.webp" width="480" alt="app light masters.create-ledger"></td>
</tr>
</table>

## light / setup.company

SSIM 0.8021

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.company.webp" width="480" alt="mock light setup.company"></td>
<td><img src="app-captures/light/setup.company.webp" width="480" alt="app light setup.company"></td>
</tr>
</table>

## dark / ai.quick-entry.idle

SSIM 0.8027

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.quick-entry.idle.webp" width="480" alt="mock dark ai.quick-entry.idle"></td>
<td><img src="app-captures/dark/ai.quick-entry.idle.webp" width="480" alt="app dark ai.quick-entry.idle"></td>
</tr>
</table>

## dark / masters.create-ledger

SSIM 0.8028

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.create-ledger.webp" width="480" alt="mock dark masters.create-ledger"></td>
<td><img src="app-captures/dark/masters.create-ledger.webp" width="480" alt="app dark masters.create-ledger"></td>
</tr>
</table>

## hc / setup.gstin-error

SSIM 0.8039

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.gstin-error.webp" width="480" alt="mock hc setup.gstin-error"></td>
<td><img src="app-captures/hc/setup.gstin-error.webp" width="480" alt="app hc setup.gstin-error"></td>
</tr>
</table>

## dark / bank.import-error

SSIM 0.8040

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/bank.import-error.webp" width="480" alt="mock dark bank.import-error"></td>
<td><img src="app-captures/dark/bank.import-error.webp" width="480" alt="app dark bank.import-error"></td>
</tr>
</table>

## hc / audit.unsupported

SSIM 0.8040

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/audit.unsupported.webp" width="480" alt="mock hc audit.unsupported"></td>
<td><img src="app-captures/hc/audit.unsupported.webp" width="480" alt="app hc audit.unsupported"></td>
</tr>
</table>

## dark / home.empty

SSIM 0.8045

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/home.empty.webp" width="480" alt="mock dark home.empty"></td>
<td><img src="app-captures/dark/home.empty.webp" width="480" alt="app dark home.empty"></td>
</tr>
</table>

## light / home.loading

SSIM 0.8074

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/home.loading.webp" width="480" alt="mock light home.loading"></td>
<td><img src="app-captures/light/home.loading.webp" width="480" alt="app light home.loading"></td>
</tr>
</table>

## light / audit.loading

SSIM 0.8078

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/audit.loading.webp" width="480" alt="mock light audit.loading"></td>
<td><img src="app-captures/light/audit.loading.webp" width="480" alt="app light audit.loading"></td>
</tr>
</table>

## light / home.empty

SSIM 0.8089

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/home.empty.webp" width="480" alt="mock light home.empty"></td>
<td><img src="app-captures/light/home.empty.webp" width="480" alt="app light home.empty"></td>
</tr>
</table>

## hc / gst.recon-empty

SSIM 0.8099

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.recon-empty.webp" width="480" alt="mock hc gst.recon-empty"></td>
<td><img src="app-captures/hc/gst.recon-empty.webp" width="480" alt="app hc gst.recon-empty"></td>
</tr>
</table>

## light / bank.import-error

SSIM 0.8104

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/bank.import-error.webp" width="480" alt="mock light bank.import-error"></td>
<td><img src="app-captures/light/bank.import-error.webp" width="480" alt="app light bank.import-error"></td>
</tr>
</table>

## light / profit-and-loss.loading

SSIM 0.8112

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/profit-and-loss.loading.webp" width="480" alt="mock light profit-and-loss.loading"></td>
<td><img src="app-captures/light/profit-and-loss.loading.webp" width="480" alt="app light profit-and-loss.loading"></td>
</tr>
</table>

## hc / ai.purchase-bill.error

SSIM 0.8138

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.purchase-bill.error.webp" width="480" alt="mock hc ai.purchase-bill.error"></td>
<td><img src="app-captures/hc/ai.purchase-bill.error.webp" width="480" alt="app hc ai.purchase-bill.error"></td>
</tr>
</table>

## hc / day-book.empty

SSIM 0.8140

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/day-book.empty.webp" width="480" alt="mock hc day-book.empty"></td>
<td><img src="app-captures/hc/day-book.empty.webp" width="480" alt="app hc day-book.empty"></td>
</tr>
</table>

## hc / setup.ai

SSIM 0.8148

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.ai.webp" width="480" alt="mock hc setup.ai"></td>
<td><img src="app-captures/hc/setup.ai.webp" width="480" alt="app hc setup.ai"></td>
</tr>
</table>

## dark / bank.empty

SSIM 0.8159

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/bank.empty.webp" width="480" alt="mock dark bank.empty"></td>
<td><img src="app-captures/dark/bank.empty.webp" width="480" alt="app dark bank.empty"></td>
</tr>
</table>

## light / bank.empty

SSIM 0.8163

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/bank.empty.webp" width="480" alt="mock light bank.empty"></td>
<td><img src="app-captures/light/bank.empty.webp" width="480" alt="app light bank.empty"></td>
</tr>
</table>

## hc / ai.purchase-bill.choose

SSIM 0.8165

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/ai.purchase-bill.choose.webp" width="480" alt="mock hc ai.purchase-bill.choose"></td>
<td><img src="app-captures/hc/ai.purchase-bill.choose.webp" width="480" alt="app hc ai.purchase-bill.choose"></td>
</tr>
</table>

## hc / bank-chooser.default

SSIM 0.8169

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/bank-chooser.default.webp" width="480" alt="mock hc bank-chooser.default"></td>
<td><img src="app-captures/hc/bank-chooser.default.webp" width="480" alt="app hc bank-chooser.default"></td>
</tr>
</table>

## dark / gst.recon-locked

SSIM 0.8181

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.recon-locked.webp" width="480" alt="mock dark gst.recon-locked"></td>
<td><img src="app-captures/dark/gst.recon-locked.webp" width="480" alt="app dark gst.recon-locked"></td>
</tr>
</table>

## light / gst.recon-locked

SSIM 0.8190

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.recon-locked.webp" width="480" alt="mock light gst.recon-locked"></td>
<td><img src="app-captures/light/gst.recon-locked.webp" width="480" alt="app light gst.recon-locked"></td>
</tr>
</table>

## dark / setup.gstin-error

SSIM 0.8204

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.gstin-error.webp" width="480" alt="mock dark setup.gstin-error"></td>
<td><img src="app-captures/dark/setup.gstin-error.webp" width="480" alt="app dark setup.gstin-error"></td>
</tr>
</table>

## hc / review.empty

SSIM 0.8207

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/review.empty.webp" width="480" alt="mock hc review.empty"></td>
<td><img src="app-captures/hc/review.empty.webp" width="480" alt="app hc review.empty"></td>
</tr>
</table>

## light / balance-sheet.loading

SSIM 0.8214

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/balance-sheet.loading.webp" width="480" alt="mock light balance-sheet.loading"></td>
<td><img src="app-captures/light/balance-sheet.loading.webp" width="480" alt="app light balance-sheet.loading"></td>
</tr>
</table>

## light / setup.gstin-error

SSIM 0.8223

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.gstin-error.webp" width="480" alt="mock light setup.gstin-error"></td>
<td><img src="app-captures/light/setup.gstin-error.webp" width="480" alt="app light setup.gstin-error"></td>
</tr>
</table>

## light / audit.integrity

SSIM 0.8234

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/audit.integrity.webp" width="480" alt="mock light audit.integrity"></td>
<td><img src="app-captures/light/audit.integrity.webp" width="480" alt="app light audit.integrity"></td>
</tr>
</table>

## hc / gst.recon-error

SSIM 0.8234

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.recon-error.webp" width="480" alt="mock hc gst.recon-error"></td>
<td><img src="app-captures/hc/gst.recon-error.webp" width="480" alt="app hc gst.recon-error"></td>
</tr>
</table>

## light / stock-summary.loading

SSIM 0.8255

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/stock-summary.loading.webp" width="480" alt="mock light stock-summary.loading"></td>
<td><img src="app-captures/light/stock-summary.loading.webp" width="480" alt="app light stock-summary.loading"></td>
</tr>
</table>

## hc / setup.keyboard

SSIM 0.8257

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.keyboard.webp" width="480" alt="mock hc setup.keyboard"></td>
<td><img src="app-captures/hc/setup.keyboard.webp" width="480" alt="app hc setup.keyboard"></td>
</tr>
</table>

## light / masters.create-item

SSIM 0.8259

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.create-item.webp" width="480" alt="mock light masters.create-item"></td>
<td><img src="app-captures/light/masters.create-item.webp" width="480" alt="app light masters.create-item"></td>
</tr>
</table>

## light / outstanding.loading

SSIM 0.8266

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/outstanding.loading.webp" width="480" alt="mock light outstanding.loading"></td>
<td><img src="app-captures/light/outstanding.loading.webp" width="480" alt="app light outstanding.loading"></td>
</tr>
</table>

## hc / gst.gstr3b-unsupported

SSIM 0.8268

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.gstr3b-unsupported.webp" width="480" alt="mock hc gst.gstr3b-unsupported"></td>
<td><img src="app-captures/hc/gst.gstr3b-unsupported.webp" width="480" alt="app hc gst.gstr3b-unsupported"></td>
</tr>
</table>

## hc / setup.welcome

SSIM 0.8272

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.welcome.webp" width="480" alt="mock hc setup.welcome"></td>
<td><img src="app-captures/hc/setup.welcome.webp" width="480" alt="app hc setup.welcome"></td>
</tr>
</table>

## light / trial-balance.loading

SSIM 0.8273

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/trial-balance.loading.webp" width="480" alt="mock light trial-balance.loading"></td>
<td><img src="app-captures/light/trial-balance.loading.webp" width="480" alt="app light trial-balance.loading"></td>
</tr>
</table>

## dark / profit-and-loss.loading

SSIM 0.8275

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/profit-and-loss.loading.webp" width="480" alt="mock dark profit-and-loss.loading"></td>
<td><img src="app-captures/dark/profit-and-loss.loading.webp" width="480" alt="app dark profit-and-loss.loading"></td>
</tr>
</table>

## dark / masters.create-item

SSIM 0.8295

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.create-item.webp" width="480" alt="mock dark masters.create-item"></td>
<td><img src="app-captures/dark/masters.create-item.webp" width="480" alt="app dark masters.create-item"></td>
</tr>
</table>

## dark / audit.integrity

SSIM 0.8303

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/audit.integrity.webp" width="480" alt="mock dark audit.integrity"></td>
<td><img src="app-captures/dark/audit.integrity.webp" width="480" alt="app dark audit.integrity"></td>
</tr>
</table>

## hc / backup.updates-off

SSIM 0.8307

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/backup.updates-off.webp" width="480" alt="mock hc backup.updates-off"></td>
<td><img src="app-captures/hc/backup.updates-off.webp" width="480" alt="app hc backup.updates-off"></td>
</tr>
</table>

## dark / home.loading

SSIM 0.8308

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/home.loading.webp" width="480" alt="mock dark home.loading"></td>
<td><img src="app-captures/dark/home.loading.webp" width="480" alt="app dark home.loading"></td>
</tr>
</table>

## light / audit.unsupported

SSIM 0.8320

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/audit.unsupported.webp" width="480" alt="mock light audit.unsupported"></td>
<td><img src="app-captures/light/audit.unsupported.webp" width="480" alt="app light audit.unsupported"></td>
</tr>
</table>

## dark / masters.items-empty

SSIM 0.8320

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.items-empty.webp" width="480" alt="mock dark masters.items-empty"></td>
<td><img src="app-captures/dark/masters.items-empty.webp" width="480" alt="app dark masters.items-empty"></td>
</tr>
</table>

## dark / audit.unsupported

SSIM 0.8321

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/audit.unsupported.webp" width="480" alt="mock dark audit.unsupported"></td>
<td><img src="app-captures/dark/audit.unsupported.webp" width="480" alt="app dark audit.unsupported"></td>
</tr>
</table>

## hc / setup.rail

SSIM 0.8330

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.rail.webp" width="480" alt="mock hc setup.rail"></td>
<td><img src="app-captures/hc/setup.rail.webp" width="480" alt="app hc setup.rail"></td>
</tr>
</table>

## dark / masters.empty

SSIM 0.8334

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/masters.empty.webp" width="480" alt="mock dark masters.empty"></td>
<td><img src="app-captures/dark/masters.empty.webp" width="480" alt="app dark masters.empty"></td>
</tr>
</table>

## light / ai.purchase-bill.choose

SSIM 0.8342

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.purchase-bill.choose.webp" width="480" alt="mock light ai.purchase-bill.choose"></td>
<td><img src="app-captures/light/ai.purchase-bill.choose.webp" width="480" alt="app light ai.purchase-bill.choose"></td>
</tr>
</table>

## hc / hi.setup.welcome

SSIM 0.8347

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/hi.setup.welcome.webp" width="480" alt="mock hc hi.setup.welcome"></td>
<td><img src="app-captures/hc/hi.setup.welcome.webp" width="480" alt="app hc hi.setup.welcome"></td>
</tr>
</table>

## light / gst.recon-empty

SSIM 0.8350

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.recon-empty.webp" width="480" alt="mock light gst.recon-empty"></td>
<td><img src="app-captures/light/gst.recon-empty.webp" width="480" alt="app light gst.recon-empty"></td>
</tr>
</table>

## light / masters.items-empty

SSIM 0.8354

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.items-empty.webp" width="480" alt="mock light masters.items-empty"></td>
<td><img src="app-captures/light/masters.items-empty.webp" width="480" alt="app light masters.items-empty"></td>
</tr>
</table>

## dark / gst.recon-empty

SSIM 0.8355

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.recon-empty.webp" width="480" alt="mock dark gst.recon-empty"></td>
<td><img src="app-captures/dark/gst.recon-empty.webp" width="480" alt="app dark gst.recon-empty"></td>
</tr>
</table>

## light / masters.empty

SSIM 0.8364

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/masters.empty.webp" width="480" alt="mock light masters.empty"></td>
<td><img src="app-captures/light/masters.empty.webp" width="480" alt="app light masters.empty"></td>
</tr>
</table>

## dark / balance-sheet.loading

SSIM 0.8365

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/balance-sheet.loading.webp" width="480" alt="mock dark balance-sheet.loading"></td>
<td><img src="app-captures/dark/balance-sheet.loading.webp" width="480" alt="app dark balance-sheet.loading"></td>
</tr>
</table>

## dark / ai.purchase-bill.choose

SSIM 0.8371

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.purchase-bill.choose.webp" width="480" alt="mock dark ai.purchase-bill.choose"></td>
<td><img src="app-captures/dark/ai.purchase-bill.choose.webp" width="480" alt="app dark ai.purchase-bill.choose"></td>
</tr>
</table>

## dark / review.empty

SSIM 0.8382

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/review.empty.webp" width="480" alt="mock dark review.empty"></td>
<td><img src="app-captures/dark/review.empty.webp" width="480" alt="app dark review.empty"></td>
</tr>
</table>

## dark / audit.loading

SSIM 0.8390

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/audit.loading.webp" width="480" alt="mock dark audit.loading"></td>
<td><img src="app-captures/dark/audit.loading.webp" width="480" alt="app dark audit.loading"></td>
</tr>
</table>

## dark / stock-summary.loading

SSIM 0.8414

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/stock-summary.loading.webp" width="480" alt="mock dark stock-summary.loading"></td>
<td><img src="app-captures/dark/stock-summary.loading.webp" width="480" alt="app dark stock-summary.loading"></td>
</tr>
</table>

## light / day-book.empty

SSIM 0.8420

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/day-book.empty.webp" width="480" alt="mock light day-book.empty"></td>
<td><img src="app-captures/light/day-book.empty.webp" width="480" alt="app light day-book.empty"></td>
</tr>
</table>

## dark / outstanding.loading

SSIM 0.8424

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/outstanding.loading.webp" width="480" alt="mock dark outstanding.loading"></td>
<td><img src="app-captures/dark/outstanding.loading.webp" width="480" alt="app dark outstanding.loading"></td>
</tr>
</table>

## light / review.empty

SSIM 0.8424

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/review.empty.webp" width="480" alt="mock light review.empty"></td>
<td><img src="app-captures/light/review.empty.webp" width="480" alt="app light review.empty"></td>
</tr>
</table>

## dark / day-book.empty

SSIM 0.8428

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/day-book.empty.webp" width="480" alt="mock dark day-book.empty"></td>
<td><img src="app-captures/dark/day-book.empty.webp" width="480" alt="app dark day-book.empty"></td>
</tr>
</table>

## dark / trial-balance.loading

SSIM 0.8431

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/trial-balance.loading.webp" width="480" alt="mock dark trial-balance.loading"></td>
<td><img src="app-captures/dark/trial-balance.loading.webp" width="480" alt="app dark trial-balance.loading"></td>
</tr>
</table>

## light / setup.ai

SSIM 0.8451

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.ai.webp" width="480" alt="mock light setup.ai"></td>
<td><img src="app-captures/light/setup.ai.webp" width="480" alt="app light setup.ai"></td>
</tr>
</table>

## dark / setup.ai

SSIM 0.8468

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.ai.webp" width="480" alt="mock dark setup.ai"></td>
<td><img src="app-captures/dark/setup.ai.webp" width="480" alt="app dark setup.ai"></td>
</tr>
</table>

## dark / ai.purchase-bill.error

SSIM 0.8475

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/ai.purchase-bill.error.webp" width="480" alt="mock dark ai.purchase-bill.error"></td>
<td><img src="app-captures/dark/ai.purchase-bill.error.webp" width="480" alt="app dark ai.purchase-bill.error"></td>
</tr>
</table>

## dark / gst.recon-error

SSIM 0.8482

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.recon-error.webp" width="480" alt="mock dark gst.recon-error"></td>
<td><img src="app-captures/dark/gst.recon-error.webp" width="480" alt="app dark gst.recon-error"></td>
</tr>
</table>

## light / setup.welcome

SSIM 0.8484

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.welcome.webp" width="480" alt="mock light setup.welcome"></td>
<td><img src="app-captures/light/setup.welcome.webp" width="480" alt="app light setup.welcome"></td>
</tr>
</table>

## dark / setup.keyboard

SSIM 0.8496

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.keyboard.webp" width="480" alt="mock dark setup.keyboard"></td>
<td><img src="app-captures/dark/setup.keyboard.webp" width="480" alt="app dark setup.keyboard"></td>
</tr>
</table>

## light / ai.purchase-bill.error

SSIM 0.8497

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/ai.purchase-bill.error.webp" width="480" alt="mock light ai.purchase-bill.error"></td>
<td><img src="app-captures/light/ai.purchase-bill.error.webp" width="480" alt="app light ai.purchase-bill.error"></td>
</tr>
</table>

## light / setup.keyboard

SSIM 0.8498

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.keyboard.webp" width="480" alt="mock light setup.keyboard"></td>
<td><img src="app-captures/light/setup.keyboard.webp" width="480" alt="app light setup.keyboard"></td>
</tr>
</table>

## light / bank-chooser.default

SSIM 0.8510

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/bank-chooser.default.webp" width="480" alt="mock light bank-chooser.default"></td>
<td><img src="app-captures/light/bank-chooser.default.webp" width="480" alt="app light bank-chooser.default"></td>
</tr>
</table>

## hc / start.recent

SSIM 0.8517

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/start.recent.webp" width="480" alt="mock hc start.recent"></td>
<td><img src="app-captures/hc/start.recent.webp" width="480" alt="app hc start.recent"></td>
</tr>
</table>

## dark / bank-chooser.default

SSIM 0.8519

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/bank-chooser.default.webp" width="480" alt="mock dark bank-chooser.default"></td>
<td><img src="app-captures/dark/bank-chooser.default.webp" width="480" alt="app dark bank-chooser.default"></td>
</tr>
</table>

## dark / gst.gstr3b-unsupported

SSIM 0.8521

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.gstr3b-unsupported.webp" width="480" alt="mock dark gst.gstr3b-unsupported"></td>
<td><img src="app-captures/dark/gst.gstr3b-unsupported.webp" width="480" alt="app dark gst.gstr3b-unsupported"></td>
</tr>
</table>

## light / gst.gstr3b-unsupported

SSIM 0.8522

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.gstr3b-unsupported.webp" width="480" alt="mock light gst.gstr3b-unsupported"></td>
<td><img src="app-captures/light/gst.gstr3b-unsupported.webp" width="480" alt="app light gst.gstr3b-unsupported"></td>
</tr>
</table>

## dark / setup.welcome

SSIM 0.8524

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.welcome.webp" width="480" alt="mock dark setup.welcome"></td>
<td><img src="app-captures/dark/setup.welcome.webp" width="480" alt="app dark setup.welcome"></td>
</tr>
</table>

## light / gst.recon-error

SSIM 0.8531

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.recon-error.webp" width="480" alt="mock light gst.recon-error"></td>
<td><img src="app-captures/light/gst.recon-error.webp" width="480" alt="app light gst.recon-error"></td>
</tr>
</table>

## dark / backup.updates-off

SSIM 0.8537

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/backup.updates-off.webp" width="480" alt="mock dark backup.updates-off"></td>
<td><img src="app-captures/dark/backup.updates-off.webp" width="480" alt="app dark backup.updates-off"></td>
</tr>
</table>

## light / backup.updates-off

SSIM 0.8538

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/backup.updates-off.webp" width="480" alt="mock light backup.updates-off"></td>
<td><img src="app-captures/light/backup.updates-off.webp" width="480" alt="app light backup.updates-off"></td>
</tr>
</table>

## hc / gst.gstr1-not-loaded

SSIM 0.8553

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.gstr1-not-loaded.webp" width="480" alt="mock hc gst.gstr1-not-loaded"></td>
<td><img src="app-captures/hc/gst.gstr1-not-loaded.webp" width="480" alt="app hc gst.gstr1-not-loaded"></td>
</tr>
</table>

## light / hi.setup.welcome

SSIM 0.8557

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/hi.setup.welcome.webp" width="480" alt="mock light hi.setup.welcome"></td>
<td><img src="app-captures/light/hi.setup.welcome.webp" width="480" alt="app light hi.setup.welcome"></td>
</tr>
</table>

## light / setup.rail

SSIM 0.8562

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.rail.webp" width="480" alt="mock light setup.rail"></td>
<td><img src="app-captures/light/setup.rail.webp" width="480" alt="app light setup.rail"></td>
</tr>
</table>

## hc / setup.backup

SSIM 0.8568

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.backup.webp" width="480" alt="mock hc setup.backup"></td>
<td><img src="app-captures/hc/setup.backup.webp" width="480" alt="app hc setup.backup"></td>
</tr>
</table>

## hc / gst.gstr3b-not-loaded

SSIM 0.8572

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/gst.gstr3b-not-loaded.webp" width="480" alt="mock hc gst.gstr3b-not-loaded"></td>
<td><img src="app-captures/hc/gst.gstr3b-not-loaded.webp" width="480" alt="app hc gst.gstr3b-not-loaded"></td>
</tr>
</table>

## dark / hi.setup.welcome

SSIM 0.8592

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/hi.setup.welcome.webp" width="480" alt="mock dark hi.setup.welcome"></td>
<td><img src="app-captures/dark/hi.setup.welcome.webp" width="480" alt="app dark hi.setup.welcome"></td>
</tr>
</table>

## dark / setup.rail

SSIM 0.8609

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.rail.webp" width="480" alt="mock dark setup.rail"></td>
<td><img src="app-captures/dark/setup.rail.webp" width="480" alt="app dark setup.rail"></td>
</tr>
</table>

## light / setup.backup

SSIM 0.8645

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.backup.webp" width="480" alt="mock light setup.backup"></td>
<td><img src="app-captures/light/setup.backup.webp" width="480" alt="app light setup.backup"></td>
</tr>
</table>

## light / gst.gstr1-not-loaded

SSIM 0.8672

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.gstr1-not-loaded.webp" width="480" alt="mock light gst.gstr1-not-loaded"></td>
<td><img src="app-captures/light/gst.gstr1-not-loaded.webp" width="480" alt="app light gst.gstr1-not-loaded"></td>
</tr>
</table>

## light / gst.gstr3b-not-loaded

SSIM 0.8676

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/gst.gstr3b-not-loaded.webp" width="480" alt="mock light gst.gstr3b-not-loaded"></td>
<td><img src="app-captures/light/gst.gstr3b-not-loaded.webp" width="480" alt="app light gst.gstr3b-not-loaded"></td>
</tr>
</table>

## dark / setup.backup

SSIM 0.8677

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.backup.webp" width="480" alt="mock dark setup.backup"></td>
<td><img src="app-captures/dark/setup.backup.webp" width="480" alt="app dark setup.backup"></td>
</tr>
</table>

## dark / gst.gstr1-not-loaded

SSIM 0.8685

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.gstr1-not-loaded.webp" width="480" alt="mock dark gst.gstr1-not-loaded"></td>
<td><img src="app-captures/dark/gst.gstr1-not-loaded.webp" width="480" alt="app dark gst.gstr1-not-loaded"></td>
</tr>
</table>

## dark / gst.gstr3b-not-loaded

SSIM 0.8690

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/gst.gstr3b-not-loaded.webp" width="480" alt="mock dark gst.gstr3b-not-loaded"></td>
<td><img src="app-captures/dark/gst.gstr3b-not-loaded.webp" width="480" alt="app dark gst.gstr3b-not-loaded"></td>
</tr>
</table>

## hc / setup.done

SSIM 0.8700

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/hc/setup.done.webp" width="480" alt="mock hc setup.done"></td>
<td><img src="app-captures/hc/setup.done.webp" width="480" alt="app hc setup.done"></td>
</tr>
</table>

## dark / start.recent

SSIM 0.8725

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/start.recent.webp" width="480" alt="mock dark start.recent"></td>
<td><img src="app-captures/dark/start.recent.webp" width="480" alt="app dark start.recent"></td>
</tr>
</table>

## light / start.recent

SSIM 0.8732

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/start.recent.webp" width="480" alt="mock light start.recent"></td>
<td><img src="app-captures/light/start.recent.webp" width="480" alt="app light start.recent"></td>
</tr>
</table>

## light / setup.done

SSIM 0.8966

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/light/setup.done.webp" width="480" alt="mock light setup.done"></td>
<td><img src="app-captures/light/setup.done.webp" width="480" alt="app light setup.done"></td>
</tr>
</table>

## dark / setup.done

SSIM 0.8984

<table>
<tr><th>Mock</th><th>App</th></tr>
<tr>
<td><img src="mock-v2/captures/states/dark/setup.done.webp" width="480" alt="mock dark setup.done"></td>
<td><img src="app-captures/dark/setup.done.webp" width="480" alt="app dark setup.done"></td>
</tr>
</table>

## Mock states with no app capture yet

### light (39)

- `ai.ask.loading`
- `hi.payment.account-list`
- `ask.result`
- `ask.loading`
- `ask.low-confidence`
- `ask.unsupported`
- `ask.offline-ai`
- `purchase-bill.choose`
- `purchase-bill.ledger-lines`
- `purchase-bill.rejected`
- `purchase-bill.reading`
- `purchase-bill.proposal`
- `purchase-bill.low-confidence`
- `purchase-bill.mismatch`
- `purchase-bill.duplicate`
- `purchase-bill.offline-ai`
- `purchase-bill.cloud-consent`
- `purchase-bill.error`
- `purchase-bill.accepted`
- `quick-entry.proposal`
- `quick-entry.hinglish`
- `quick-entry.unknown-ledger`
- `quick-entry.loading`
- `quick-entry.rejected`
- `balance-sheet.voucher-einvoice-exported`
- `gst.recon-explain`
- `bank.ai-suggestions`
- `bank.suggested-ledger`
- `masters.suggested-ledger`
- `backup.restored`
- `backup.restore-refused`
- `backup.upgraded`
- `backup.updates-available`
- `backup.updates-none`
- `backup.updates-failed`
- `profit-and-loss.print-error`
- `gst.gstr1-failed`
- `audit.failed`
- `gst.gstr1-unsupported`

### dark (39)

- `ai.ask.loading`
- `hi.payment.account-list`
- `ask.result`
- `ask.loading`
- `ask.low-confidence`
- `ask.unsupported`
- `ask.offline-ai`
- `purchase-bill.choose`
- `purchase-bill.ledger-lines`
- `purchase-bill.rejected`
- `purchase-bill.reading`
- `purchase-bill.proposal`
- `purchase-bill.low-confidence`
- `purchase-bill.mismatch`
- `purchase-bill.duplicate`
- `purchase-bill.offline-ai`
- `purchase-bill.cloud-consent`
- `purchase-bill.error`
- `purchase-bill.accepted`
- `quick-entry.proposal`
- `quick-entry.hinglish`
- `quick-entry.unknown-ledger`
- `quick-entry.loading`
- `quick-entry.rejected`
- `balance-sheet.voucher-einvoice-exported`
- `gst.recon-explain`
- `bank.ai-suggestions`
- `bank.suggested-ledger`
- `masters.suggested-ledger`
- `backup.restored`
- `backup.restore-refused`
- `backup.upgraded`
- `backup.updates-available`
- `backup.updates-none`
- `backup.updates-failed`
- `profit-and-loss.print-error`
- `gst.gstr1-failed`
- `audit.failed`
- `gst.gstr1-unsupported`

### hc (39)

- `ai.ask.loading`
- `hi.payment.account-list`
- `ask.result`
- `ask.loading`
- `ask.low-confidence`
- `ask.unsupported`
- `ask.offline-ai`
- `purchase-bill.choose`
- `purchase-bill.ledger-lines`
- `purchase-bill.rejected`
- `purchase-bill.reading`
- `purchase-bill.proposal`
- `purchase-bill.low-confidence`
- `purchase-bill.mismatch`
- `purchase-bill.duplicate`
- `purchase-bill.offline-ai`
- `purchase-bill.cloud-consent`
- `purchase-bill.error`
- `purchase-bill.accepted`
- `quick-entry.proposal`
- `quick-entry.hinglish`
- `quick-entry.unknown-ledger`
- `quick-entry.loading`
- `quick-entry.rejected`
- `balance-sheet.voucher-einvoice-exported`
- `gst.recon-explain`
- `bank.ai-suggestions`
- `bank.suggested-ledger`
- `masters.suggested-ledger`
- `backup.restored`
- `backup.restore-refused`
- `backup.upgraded`
- `backup.updates-available`
- `backup.updates-none`
- `backup.updates-failed`
- `profit-and-loss.print-error`
- `gst.gstr1-failed`
- `audit.failed`
- `gst.gstr1-unsupported`

