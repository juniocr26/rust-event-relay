[English](../../en/adr/001-use-rust-for-the-relay.md) | [README](../../../README.pt-BR.md)

# ADR 001 — Usar Rust para o relay

Estado: aceito. Data: 2026-10-07.

## Contexto

O problema do portfólio é entrega distribuída de eventos sob falhas. Um serviço de longa duração precisará de limites explícitos de recursos, estado concorrente seguro e ciclo de vida recuperável. Também é uma oportunidade de estudar programação de sistemas; colecionar linguagens não é o objetivo.

## Decisão

Usar Rust estável e Cargo. Ownership e borrowing tornam explícitos os tempos de vida e compartilhamento; segurança de memória em Rust seguro reduz certas falhas de acesso à memória. O modelo de concorrência ajuda a expressar transferências seguras de propriedade; Tokio e Axum oferecem ecossistema async estabelecido para rede e ciclo de vida. O projeto inicial não usa código unsafe.

## Consequências e alternativas

Rust exige aprender ownership, cancelamento async e desenho de erros; tempo de compilação e complexidade do ecossistema são custos reais. Segurança de memória não impede perda de entrega, deadlocks, condições de corrida lógicas, filas ilimitadas ou tentativas incorretas. Isso exige engenharia e testes de falha. Go, Java e outras linguagens maduras também poderiam implementar bem o serviço; Rust não é automaticamente mais rápido nem superior. A adequação a serviços de infraestrutura de longa duração e o valor educativo motivam a escolha, sem alegações de desempenho sem medição. Manter dependências e abstrações proporcionais ao comportamento demonstrado.
