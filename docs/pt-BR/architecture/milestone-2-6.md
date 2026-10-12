[English](../../en/architecture/milestone-2-6.md) | [README](../../../README.pt-BR.md)

# Marco 2.6 — Experimentos integrados de recuperação

Partida: `d6f529b`, árvore limpa. O relatório 2.5 solicitado não existe neste checkout. Código versionado, marcos 2.3–2.5 e ADR 007 serviram de base; nenhum AGENTS.md aplicável foi encontrado nos projetos/ancestrais. Estado compartilhado observado: HTTP RUNNING, relay STOPPED, migração de posse pendente. Sem autorização de rollout compartilhado.

## Matriz definida antes dos testes

A matriz canônica em inglês foi escrita antes da implementação; esta página mantém o mesmo escopo em português. Todos os casos usam banco exato `relay_it_<UUID>` e exchange/queue `relay.it.<UUID>.*`. Migrações são aplicadas apenas nesses bancos. ACK do observador é explícito. Polling limitado consulta o relógio do PostgreSQL: intervalos de consulta não são prova de expiração.

| Caso | Fronteira, falha e sincronização | Banco/broker esperados; recuperação e limite |
| --- | --- | --- |
| Baseline | Binário relay original; polling de processed | Count 1, processed_at presente, posse totalmente limpa; envelope/message_id originais; ACK e TERM. Fixture não comprova atomicidade produtor/outbox nem efeito de consumidor. |
| A | Executável de teste com adapters e caso de uso reais; wrapper do publisher sinaliza arquivo ao entrar, depois da aquisição commitada e antes do envio; SIGKILL no filho exato | Pending/count 1/token durável; zero mensagens. Expiração real; reinício com binário original, token novo/count 2; mensagem original e processed. Fronteira de crash instrumentada, não binário original. |
| B | Wrapper delega publicação real confirmada e sinaliza arquivo antes de retornar ao caso de uso; SIGKILL | Pending/count 1; primeira mensagem aceita sob contrato publisher; completion ainda não começou. Expiração real, binário original → segunda mensagem com mesmo ID e processed/count 2. Janela explícita de duplicação. |
| C | Proxy AMQP existente retém bytes broker→cliente após setup; timeout real | Uncertain/adapter aposentado/LeaseRetained, pending/count 1; aceitação precisa ser observada independentemente. Conexão nova não repete claim; expiração real/token novo/count 2 → confirmação e Applied. Proxy de bytes, não perda seletiva de frames AMQP. |
| D | Proxy PostgreSQL existente descarta CommandComplete COMMIT apenas na conexão usada por completion, por repository de encaminhamento | Publicação confirmada; CommitUncertain; pool direto pode observar processed. Sem release/republish; reinício não adquire nem duplica. Injeção ocorre depois da conclusão COMMIT observada, sem generalizar todo commit incerto. |
| Completion comum falha | Constraint trigger diferido, exclusivamente no banco isolado, rejeita processed no COMMIT | Mensagem confirmada, OperationFailed, pending/lease retido. Remover trigger e aguardar expiração real; proprietário novo consegue concluir, com duplicação possível. |
| Dois workers | Dois binários originais; trigger de completion aguarda advisory lock da fixture; observar tokens duráveis e duas esperas reais | Dois IDs pending/count 1/tokens distintos/leases vivos; mensagens confirmadas antes da conclusão. Soltar lock → ambos processed. Não prova fairness, ordem de negócio ou todas as corridas. |
| Owner antigo | A adquire, lease expira pelo relógio real; B adquire com token novo | A complete/release retornam OwnershipLost; B preserva autoridade e conclui depois de confirmação. Publisher anterior ainda consegue publicar envelope original: tokens SQL não fazem fencing no RabbitMQ. |
| Publisher indisponível | Binário original contra proxy de handshake bloqueado; observar log de falha antes de restaurar forwarding para conexões futuras | Pending/count 0/queue vazia. Restaurar disponibilidade → count 1/processed. TERM em idle e INT em reconexão limitados. Nenhum serviço compartilhado interrompido. |
| Desconexão durante entrega | Worker real com lifecycle de construção instrumentado; reter confirms, observar primeira mensagem e abortar apenas proxy próprio | Uncertain, pending/count 1/lease vivo; exatamente duas conexões observadas. Substituição não repete claim; expiração permite count 2/processed e mensagem duplicada original. |
| Banco bloqueado/shutdown | Advisory lock isolado bloqueia completion; deadlines da sessão ou TERM durante espera | Sem processed prematuro; mensagem confirmada pode existir. Soltar lock, aguardar expiração e reiniciar. Deadlines cooperativos não cobrem power loss ou kernel parado. |

## Resultados observados — 2026-10-12

O transcript sanitizado (historical artifact unavailable in this checkout) registra nomes exatos de bancos/topologias, cada ID inserido, IDs observados no broker e snapshots duráveis finais sem payload. Há dez testes de experimento e uma entrada interna de processo filho; A/B compartilham uma função com duas fixtures. O teste de cleanup gera um panic intencional e verifica recursos ausentes independentemente; seus diagnósticos esperados não significam falha da suíte.

| Caso | Observação efetiva |
| --- | --- |
| Baseline existente, reforçado | Count 1/processed_at presente/todos os campos de posse NULL; envelope/payload/message_id corretos; ACK e TERM com sucesso. |
| A | Pending/count 1 inalterado após SIGKILL; nenhuma mensagem. Expiração real; binário reiniciado bloqueado na conclusão apresenta token novo/count 2. Mensagem original; soltar barreira → processed/count 2 e posse limpa. |
| B | Primeira mensagem confirmada e ACK do observador antes do kill; pending/count 1 inalterado. Expiração real; binário com token novo/count 2, segunda mensagem do mesmo ID, processed/count 2. |
| C | Uncertain/LeaseRetained e pending/count 1; primeira mensagem aceita observada separadamente. Conexão nova retorna NoEvent durante lease vivo e não envia de novo. Expiração/token novo → count 2/Applied e segunda mensagem original. |
| Desconexão/worker | Primeira mensagem observada; proxy próprio abortado durante espera de confirmação. Uncertain e segunda conexão enquanto lease count 1 ainda vive. Depois da elegibilidade, count 2/processed e segunda mensagem original. |
| D | Publicação confirmada, completion CommitUncertain; proxy observa COMMIT; pool direto observa processed/count 1 sem posse. Acquire direto None e binário reiniciado chega a NoEvent; sem mensagem adicional/count novo. |
| Rejeição/owner antigo | COMMIT de processed rejeitado, OperationFailed; pending/count 1 e primeira mensagem. Expiração real → B/token novo/count 2. A complete/release rejeitados, token B preservado. Publisher anterior ainda envia envelope original; B envia/confirma/conclui. Três mensagens com ID original demonstram ausência de fencing externo. |
| Dois binários | Dois tokens distintos duráveis, count 1 e lease vivo; duas esperas de lock. Conjunto exato de IDs observados igual ao inserido. Após unlock, ambos processed/count 1; TERM/INT e fila sem mensagem extra. |
| Indisponibilidade | Falha de handshake observada; count 0/fila vazia. Forwarding restaurado → mensagem original/processed/count 1. TERM idle e INT reconnect dentro de quatro segundos. |
| Deadline SQL | lock_timeout 100 ms/statement_timeout 300 ms na conexão de completion; OperationFailed em menos de dois segundos; mensagem confirmada/pending/count 1. Unlock/expiração real/reinício → processed/count 2 e duplicata original. |
| Shutdown em voo | Binário confirma e bloqueia no trigger. TERM cancela após drain de 200 ms, encerra dentro de quatro segundos sob cleanup configurado de 2000 ms. Pending/count 1; unlock, expiração real do lease de oito segundos e reinício → processed/count 2 e duplicata original. |

Nenhum defeito produtivo foi confirmado; source/schema/configuração de produção não mudaram. Correções de implementação ficaram nos imports, coordenação, ordem do observador e cleanup do harness. Proxies existentes foram extraídos para [support/faults.rs](../../../backend/tests/support/faults.rs), preservando comportamento e regressões históricas. O baseline passou a conferir todos os campos limpos e message_id.

## Reprodução, duração e cleanup

Reutilize app/PostgreSQL/RabbitMQ configurados; não aplique migração compartilhada. Credenciais precisam de CREATE DATABASE e permissão de topologia. Proxies locais usam conexões sem TLS. Execute no Linux com `kill`, já disponível na imagem Docker. Binário/helper não geram descendentes; somente filhos diretos rastreados são sinalizados e aguardados.

```bash
docker compose exec -T app cargo test --locked --test worker_smoke -- --ignored --test-threads=1
docker compose exec -T app cargo test --locked --test integrated_recovery -- --ignored --test-threads=1 --nocapture
```

Suíte nova: aproximadamente 27 segundos locais depois da compilação; três smokes existentes: aproximadamente 23 segundos. Hardware/compilação podem acrescentar tempo. Observações: oito segundos; tarefa de caso: 30 segundos; cleanup broker: oito segundos; harness de banco: 45 segundos. Assert de saída TERM/INT: quatro segundos incluindo sinal/reap, diferente do budget cooperativo produtivo. Experimentos novos não editam timestamps de lease; regressões antigas incluem fixtures sintéticas identificadas.

O [job CI](../../../.github/workflows/ci.yaml) configura ambos os serviços descartáveis, baseline do binário e recuperação. Não foi executado remotamente nesta sessão. `cargo test` padrão mantém infraestrutura opt-in. Execução nativa exige o mesmo ambiente POSTGRES/RABBITMQ e broker acessível; nunca use credenciais literais na linha de comando.

O harness externo retém filhos, proxies e diretório temporário mesmo se a tarefa de assertions falha. Mata/reap somente filhos próprios; aborta/join somente proxies próprios; exclui queue/exchange exatos (binding sai com queue); fecha observador; remove diretório exato; pool fecha e banco exato é removido/verificado. Catálogo SQL, declarações passive negativas e ausência do diretório verificam cleanup. A regressão com panic verifica ausência independente de processo, banco e topologia. ACK do observador remove mensagens intencionalmente e não representa perda do relay.

SIGKILL/power loss do harness podem deixar recursos: use exclusivamente nomes exatos impressos no transcript, confirme que filhos/conexões próprios terminaram e remova apenas esses objetos. Nunca faça exclusão por prefixo nem purge compartilhado. Logs não incluem credenciais, payloads, connection strings ou cadeia irrestrita de causas.

## Aceitação e próximo marco

A [revisão do Marco 2](../operations/milestone-2-review.md) fecha o escopo implementado sob premissas explícitas. A [validação](../testing/validation-results.md#marco-26--2026-10-12) separa execução local de CI configurado. Rollout compartilhado continua pendente, relay STOPPED/autostart=false. Marco 3 exige decisões de retries duráveis, classificação, isolamento poison, quarantine/dead-letter e replay auditável; nenhuma dessas políticas foi implementada.
