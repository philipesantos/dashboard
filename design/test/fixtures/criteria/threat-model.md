# Threat model (fixture)

## Threats and planned tests

| Threat | Attacker | Asset | Boundary | Mitigation | Test from |
| --- | --- | --- | --- | --- | --- |
| Infinite loop | T1 | A3 | B1 | Fuel | Phase 1 |
| Drawing over the frame | T1 | A2 | B4 | V (HS-01) | Phase 3 |
| Widget opens a dialog | T1 | A2, A4 | B1, B4 | V, VII (HS-01, HS-02) | Phase 3 |
| Prompt injection | T2 | A1 | B4, B5 | VI | Phase 4 |
