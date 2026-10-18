# Contrato RabbitMQ atual

Relay publica envelopes originais com ID estável em direct durável `relay.events`, chave `event`, fila `relay.local`, vhost `relay`. Mandatory/returns e confirms classificam aceitação, rejeição/unroutable conhecidos e incerteza. Leases protegem transições SQL, não envios externos. Reconexão não prova não aceitação anterior; retry pode duplicar transporte.

Consumidor de estoque implementado usa ACK manual após transação atômica de estoque/recibo. Entrada inválida/não suportada/conflitante exige quarentena confirmada gerida pela aplicação antes do ACK; falha de preservação mantém origem. Padrões: `relay.quarantine`, chave `inventory`, fila `relay.quarantine.inventory`. Ações duplicadas usam fila/chave durável `relay.demo.<UUID da ação>` e consumidores temporários limitados. dead_letter SQL difere da quarentena broker. Credenciais do broker diferem de tokens demo/operador; AMQP atual não é caminho produtivo verificado de TLS/criptografia do payload.

[Rastreio completo de evento/ACK/incerteza](../guides/implementation-study.md). Afirmações abaixo de adaptador isolado/consumidor ausente pertencem ao marco original, não ao contrato atual.

## Referência histórica (datas e escopo originais)

Admissão e instrumentação atuais: publisher/quarentena rejeitam saturação antes da serialização, sem waiters internos. Descrições datadas abaixo são históricas. [Milestone 5.1](../operations/milestone-5-1.md).

#
Worker atual, configuração, cancelamento e rollout autorizado: [Marco 2.5](../architecture/milestone-2-5.md). Seções anteriores mantêm escopo histórico.

 Integração verificada do publisher

## Marco 2.4 — 2026-10-12

O caso de uso de entrega executa aquisição reconhecida → publicação do envelope original → completion somente após aceitação confirmada. Falhas definitivas liberam uma vez; incerteza mantém lease. Resultados preservam falha principal e cleanup separadamente. HTTP continua independente; não há worker composto. Consulte [Marco 2.4](../architecture/milestone-2-4.md) para resultados, cancelamento e limites de recuperação. Próximo: 2.5 composição/operação do worker; 2.6 experimentos ponta a ponta.

Seções anteriores preservam o escopo do marco/revisão registrado.

[English](../../en/integrations/rabbitmq-contract.md) | [Português brasileiro](rabbitmq-contract.md)

Revisão estática do código: 2026-10-10. Fatos implementados, teoria geral e mudanças hipotéticas são separados abaixo. Comandos runtime não foram executados.

O adapter RabbitMQ chamável implementa o port `EventPublisher` da aplicação. Abre conexão/channel próprios, declara exchange direct durável e fila durável, vincula pela routing key configurada e habilita publisher confirms. Defaults são `relay.events`, `relay.local` e `event`. Serializa envelope em JSON, usa identidade estável do evento como message ID, envia mensagem persistente com roteamento mandatory e espera confirmação. Ack bem-sucedido sem retorno comprova aceitação pelo broker nesse contrato; não comprova consumidor, efeitos de negócio ou entrega ponta a ponta. Não há consumidor implementado.

`RabbitMqConfig` constrói URI AMQP escapada com campos separados de credencial/vhost, valida nomes/prazos e redige Debug. A configuração atual aceita `amqp`, não `amqps`; não há caminho TLS produtivo verificado. Autenticação usa credenciais/vhost do broker, não autenticação HTTP da aplicação ou cifra de payload. Compose local fornece armazenamento e endpoint de gestão; login de gestão é fronteira operacional distinta da publicação.

O mutex serializa uma publicação em andamento. Espera de admissão e publicação têm prazos separados; setup e fechamento também são limitados. Serialização/indisponibilidade, rejeição, ausência de rota e incerteza são resultados distintos. Timeout/erro/cancelamento após início do envio pode deixar aceitação desconhecida; o adapter aposenta o channel e exige reconexão explícita. Não faz retry automático. Token de lease do banco não é enviado como fencing ao broker e não impede envio externo de processo obsoleto.

Leitor PostgreSQL, ports de ownership e publisher são caminhos separados de biblioteca. `main` compõe apenas configuração/tracing/liveness HTTP. Não há transação de produtor, worker polling, SQL de aquisição/completion, efeito de consumidor ou integração de pagamento ligando-os. Aquisição → publicação → conclusão permanece orquestração hipotética. Um diagrama ligando adapters como worker em execução seria incorreto.

## Marco 3.3 — integração atual

Integração de entrega/worker implementada; seções anteriores mantêm seu escopo histórico datado. [Contratos atuais, jitter, matriz contextual, manutenção e cancelamento](../architecture/milestone-3-3.md). Scanning geral fica em 3.4, replay auditado em 3.5 e aceitação completa em 3.6; rollout compartilhado permanece pendente.
