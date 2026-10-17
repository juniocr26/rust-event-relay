[English](../../en/operations/final-audit.md) | [Guia da aplicação](application-scenarios.md)

# Auditoria final de implementação - 17/10/2026

Decisão: aceito no escopo local de portfólio acordado. Esta campanha final reexecutou as verificações na base Git `18893c865eba506e7d4a2124864920d82ce6237d`; os relatórios anteriores foram tratados como histórico. Nenhum novo defeito funcional ou bloqueador obrigatório foi confirmado. Código de runtime e migrações não precisaram de alteração. CI remoto permanece não verificado; prontidão de produção não foi estabelecida.

O PDF foi um artefato temporário de comunicação, sem requisito de instalação. As informações de aceite mantidas estão neste documento e na matriz 6.3.

## Execução atual

- Rust isolado: 102 aprovados na suíte padrão / 141 opt-ins ignorados nessa invocação; 138 entradas de integração aprovadas (136 substantivas e dois dispatchers filhos); dois opt-ins adicionais da biblioteca aprovados. Formatação, Clippy estrito para todos os targets/features, build dos bins e rustdoc sem warnings passaram.
- Capacidade limitada: um teste substantivo, 22 trials, zero resultados inexplicados; 247 amostras agregadas do broker, zero erros do observer. Total único Rust: 243 entradas aprovadas, 241 substantivas e dois helpers. Repetições da instalação limpa e targets vazios não foram somados.
- Aplicação real: 27 testes browser aprovados, incluindo 24 principais, consumer pausado, fixture unknown e API indisponível. SQL independente e sete filas de ação vazias sem consumers temporários passaram.
- Instalação limpa: 290 arquivos visíveis ao Git, novas credenciais/portas/armazenamento, nove migrações; 102 testes Rust padrão e 24 browser passaram, além de npm locked, type-check/build, scan de segredos nos assets e persistência após stop/start. SQL independente validou 13 runs, sete ações e soma de estoque 195. Recursos próprios removidos.
- Nove upgrades/checksums, no-op e rollback/reaplicação suportados passaram em banco descartável. Compose, links locais, quatro testes do checker e whitespace do diff passaram.
- Frontend principal real: inspeção GET-only; as 12 consultas copiadas rodaram em transação somente leitura. Capturas mobile/tablet/desktop foram examinadas; teclado, foco e reduced motion passaram. Foi revisão especializada com tarefas executadas no browser, não estudo formal de usabilidade.

## Preservação e limites

Contagens e fingerprints das 11 tabelas retidas permaneceram iguais. HTTP/relay/consumer principais permaneceram RUNNING, readiness 200 e hashes dos executáveis carregados iguais aos bins principais em disco. Os quatro serviços ficaram saudáveis. Filas principais com zero ready/unacknowledged; source com um consumer e ação com zero. Nenhum serviço principal foi parado para faults, credencial ou migração principal alterada, fila purgada, receipt apagado ou volume resetado.

Quatro cenários são reais na UI: entrega normal, duplicata, duplicata concorrente e rejeição antes do envio/retry restrita ao run. Crash, corrida de workers e confirmação perdida permanecem ilustrações; os testes reais dessas fronteiras rodaram somente em recursos descartáveis. Outbox processed, confirmação do broker, receipt e recebimento de ACK pelo broker continuam distintos. Unknown não prova rollback. Limites das observações e lacunas históricas estão explícitos no PDF.

TLS/HA, publicação, recuperação do broker, retenção protegida e escala ampliada continuam opcionais. A auditoria acrescenta evidências e encerramento documental sem reescrever execuções históricas nem prometer transporte exatamente uma vez.
