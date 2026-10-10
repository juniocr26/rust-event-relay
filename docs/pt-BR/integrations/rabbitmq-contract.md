# Integração verificada do publisher

[English](../../en/integrations/rabbitmq-contract.md) | [Português brasileiro](rabbitmq-contract.md)

Revisão estática do código: 2026-10-10. Fatos implementados, teoria geral e mudanças hipotéticas são separados abaixo. Comandos runtime não foram executados.

O adapter RabbitMQ chamável implementa o port `EventPublisher` da aplicação. Abre conexão/channel próprios, declara exchange direct durável e fila durável, vincula pela routing key configurada e habilita publisher confirms. Defaults são `relay.events`, `relay.local` e `event`. Serializa envelope em JSON, usa identidade estável do evento como message ID, envia mensagem persistente com roteamento mandatory e espera confirmação. Ack bem-sucedido sem retorno comprova aceitação pelo broker nesse contrato; não comprova consumidor, efeitos de negócio ou entrega ponta a ponta. Não há consumidor implementado.

`RabbitMqConfig` constrói URI AMQP escapada com campos separados de credencial/vhost, valida nomes/prazos e redige Debug. A configuração atual aceita `amqp`, não `amqps`; não há caminho TLS produtivo verificado. Autenticação usa credenciais/vhost do broker, não autenticação HTTP da aplicação ou cifra de payload. Compose local fornece armazenamento e endpoint de gestão; login de gestão é fronteira operacional distinta da publicação.

O mutex serializa uma publicação em andamento. Espera de admissão e publicação têm prazos separados; setup e fechamento também são limitados. Serialização/indisponibilidade, rejeição, ausência de rota e incerteza são resultados distintos. Timeout/erro/cancelamento após início do envio pode deixar aceitação desconhecida; o adapter aposenta o channel e exige reconexão explícita. Não faz retry automático. Token de lease do banco não é enviado como fencing ao broker e não impede envio externo de processo obsoleto.

Leitor PostgreSQL, ports de ownership e publisher são caminhos separados de biblioteca. `main` compõe apenas configuração/tracing/liveness HTTP. Não há transação de produtor, worker polling, SQL de aquisição/completion, efeito de consumidor ou integração de pagamento ligando-os. Aquisição → publicação → conclusão permanece orquestração hipotética. Um diagrama ligando adapters como worker em execução seria incorreto.
