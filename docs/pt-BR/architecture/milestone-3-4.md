[Outro idioma](../../en/architecture/milestone-3-4.md) | [ADR 008](../adr/008-durable-failure-policy.md)

# Marco 3.4 — Descoberta limitada e isolamento seguro

Implementado em 13/10/2026 sobre Rust a756797bbdd64d50671d536ae7de697a3705ce8b. O rollout local já instalou as migrations de ownership/retry e ativou os processos; relatos anteriores de parada continuam históricos. Esta alteração não cria migration nem executa ativação compartilhada. Replay pertence ao 3.5; aceitação completa, ao 3.6.

## Matriz de detecção e autoridade

Toda mutação automática exige pending atual, disponibilidade vencida, identidade não nula utilizável, ownership e histórico coerentes, revisão incrementável, ciclo/revisão esperados e lease ausente/expirada, verificados após lock. Leitura é observação. Aceitação anterior pelo broker continua possível com histórico de aquisições; esgotamento ou invalidade não provam ausência de publicação. Mesmo contador zero é histórico armazenado, não prova universal sobre escritores externos.

| Classe / evidência | Ação automática | Operador / preservação |
| --- | --- | --- |
| Restaurável, vencido, com orçamento | Valid; pode receber aquisição nova por ID | Sem isolamento; regras normais de ownership/publicação |
| Restaurável, consumo do ciclo >= máximo | acquisition_budget_exhausted após revalidação | Preserva envelope, identidade e histórico; replay exige auditoria |
| Restaurável, contador vitalício = i32::MAX | storage_counter_exhausted, precede orçamento | Política explícita de capacidade; não zerar/saturar contador |
| Restauração verificada retorna EventError tipado | invalid_stored_envelope com metadados seguros e mesma evidência atual | Espaços em nomes/IDs violam envelope-v1; mantém conteúdo original |
| Versão/representação não suportada, falha de decoder JSONB/timestamp | UnsupportedRepresentation, sem isolamento | Investigar compatibilidade/implementação; erro de decoder não prova corrupção permanente |
| Identidade nil, tuple/tempo de ownership inválido, ciclo/histórico/revisão inseguros ou sem capacidade de incremento | RepairRequired, sem inventar autoridade | Reparo auditado autorizado separadamente; preserva campos |
| Falha de consulta/pool/transporte | Erro sanitizado, sem candidato ou isolamento | Diagnóstico de infraestrutura; nova leitura limitada |
| Lease ativa, inclusive dono no orçamento final | LiveOwner, sem isolamento | Pode concluir antes da expiração; lease não revoga envio em andamento |
| Disponibilidade futura | NotDue, sem mutação | Revisitado na próxima travessia |

Versão de schema é u32 positivo, não catálogo de eventos de negócio. Valores nesse intervalo são metadados suportados sem validar schema específico do payload. Fora do intervalo/representações não suportadas não recebem atestado de corrupção permanente. Usa a mesma restauração StoredEvent verificada do leitor/adquirente, distinguindo EventError de erro privado de decodificação. Serialização de entrega isoladamente continua insuficiente para declarar poison.

## Travessia limitada

OutboxCandidateDiscovery retorna zero ou uma observação. PostgreSQL visita todas as linhas pending, incluindo futuras, com dono ativo ou metadados inseguros, em ordem UUID da chave primária. DiscoveryCursor guarda after/through: a primeira leitura observa o maior UUID da tabela; as seguintes usam id > after AND id <= through. Não usa OFFSET. Estados terminais são filtrados. Descoberta não bloqueia linhas: observa versão confirmada mesmo com lock; isolamento por ID usa SKIP LOCKED. Scanners concorrentes podem observar a mesma linha; predicados atuais resolvem a competição.

maintain_once avança antes do isolamento, inclusive para casos que exigem reparo, conflitos, erros e commits incertos. Vazio significa fim desse sufixo no snapshot, reinicia cursor e encerra travessia; não significa backlog vazio. UUIDs maiores recém-inseridos esperam a próxima passagem; inserções/alterações atrás do cursor e mutações que saltaram lock são revisitadas após retorno ao início. Reinício perde progresso local; cursor/fila não são duráveis. Limite superior fixo impede que novas chegadas prolonguem indefinidamente a faixa atual. Contenção infinita, mudanças contínuas ou reinícios repetidos não demonstram justiça/cobertura completa.

Uma consulta, no máximo uma linha/payload materializado e depois até uma transação curta de isolamento. Só evidência sanitizada e UUID/ciclo/revisão cruzam a interface; payload não fica guardado para listar candidatos. Validação ainda materializa um JSON completo, sem limite de bytes. LIMIT limita retorno/memória em quantidade de linhas, não páginas, tuplas examinadas, bytes ou latência; JSON grande e muitas linhas terminais filtradas continuam limites. Mantém deadlines de pool/statement/lock; não cria índice especulativo nem resolve o marco de limite de payload.

## Isolamento e evidência atuais

OutboxCandidateIsolator bloqueia ID exato com FOR UPDATE SKIP LOCKED e amostra clock_timestamp em consulta separada após lock. Relê envelope/ownership/retry, exige pending/vencimento/ownership elegíveis e ciclo/revisão esperados e repete a validação relevante de envelope/orçamento. Mudança da evidência tipada causa Conflict. Preserva conteúdo inválido atual; não depende de supor que escritores externos incrementem revisão ou mantenham envelope imutável para revalidar invalidade. Revisão continua necessária ao protocolo; alterações não auditadas de histórico sem revisão não podem ser reconstruídas.

UPDATE terminal existente mantém a linha dead_letter, processed_at NULL, limpa ownership autorizado e avança revisão com aritmética verificada. terminal_at/diagnostic_at usam o mesmo instante pós-lock, não ocorrência do evento nem falha histórica inferida. Motivos distinguem envelope inválido, orçamento e capacidade. failure_evidence guarda adr008-v1, fase maintenance, envelope-v1 para invalidade e UUID original como evidence_reference à linha preservada. Revisão observada é predecessora da confirmada; ciclo permanece armazenado. EventError tipado aparece em log sanitizado limitado e pode ser recalculado do conteúdo preservado; payload/erro irrestrito não são copiados. last_error e conteúdo JSONB exato são mantidos, sem alegar recuperação da formatação JSON original do produtor.

Rejeição comum faz rollback sem alteração parcial; linha ausente/obsoleta/bloqueada/ativa/alterada retorna Conflict, sem inferir sucesso anterior. CommitUncertain pode esconder estado terminal confirmado. Cancelamento pode suprimir resultado após commit; descarte de transação SQLx enfileira rollback, sem provar rollback imediato. Não há repetição cega, compensação, tarefa destacada ou reconciliação implícita. Próxima travessia observa novamente. Constraints validam formato, não autorização histórica de todos os escritores. Dono expirado pode ser isolado após publicação possível; manutenção não cerca o broker.

## Worker e compatibilidade

Até uma unidade de manutenção por iteração e pelo menos um segundo após terminar a anterior: uma tentativa de ID configurado OU uma descoberta com até um isolamento. Sem rajada para compensar intervalos perdidos, fila ilimitada, tarefas adicionais ou concorrência de manutenção. Usa mesma fronteira de drain/shutdown da entrega. Conexão, entrega e pausas interrompíveis idle/failure/reconnect podem atrasar a próxima unidade; um segundo limita frequência de admissão, não garante prazo.

Manutenção de banco fica antes da conexão do publisher: indisponibilidade do broker permite uma unidade por iteração de reconexão pausada. Não executa simultaneamente a uma operação limitada ainda pendente. Erros de manutenção não autorizam transições de entrega nem impedem tentativa elegível independente. Logs distinguem manutenção e entrega.

Observação Valid fornece no máximo uma sugestão de ID para seleção. Worker consome antes da entrega; AcquireRequest::targeting muda seleção, não concede lease nem dispensa elegibilidade/restauração/checagem pós-lock. Permite entregar trabalho válido após prefixo inseguro/não suportado sem reparar esse prefixo. Alvo ausente/obsoleto/bloqueado pode retornar NoEvent; nova travessia reobserva. Aquisição padrão mantém ordenação por disponibilidade e erro de linha selecionada malformada. Guarda só um UUID, substituível durante outage; não é fila de entrega nem garantia de justiça.

RELAY_MAINTENANCE_CANDIDATES permanece opcional/compatível: até 4096 bytes, 32 UUID:cycle:revision únicos, consumidos uma vez por execução, agora sob limite de um segundo e antes do gate do broker. IDs configurados têm prioridade temporária finita; depois começa descoberta. Reinício relê configuração, portanto remover observações obsoletas. Manutenção legada também verifica coerência de ownership antes de limpar campos. Nenhuma configuração nova ou reinício compartilhado foi executado.

## Verificação e próximos marcos

[Resultados executados](../testing/validation-results.md) separam testes determinísticos, PostgreSQL isolado e observações reais de banco/broker. Não demonstram justiça universal, durabilidade sob perda de energia, efeito de consumidor ou CI remoto. Constraints de produção intactas; fixtures corrompidas removem constraints apenas do banco descartável exato.

Pré-requisitos 3.5: intenção explícita autenticada/autorizada de replay, revisão dead_letter esperada, identidade única durável de solicitação/auditoria, novo ciclo/baseline seguro em concorrência, contador vitalício e ID/envelope originais preservados, capacidade de ciclo/revisão/contador verificada, política de validação/reparo para envelopes inválidos/não suportados, observação independente de COMMIT incerto/cancelamento e testes contra requisições ativas/não terminais/obsoletas/repetidas não autorizadas. Replay não reconstrói nem desfaz efeitos anteriores do broker/consumidor. Identidade/histórico inseguros exigem protocolo de reparo auditado separado. 3.6 fecha aceitação integrada; frontend simulado e idempotência do consumidor permanecem iguais.

Referência: [locking de SELECT no PostgreSQL](https://www.postgresql.org/docs/current/sql-select.html): SKIP LOCKED salta contenção de lock de linha, não elimina todo lock de tabela nem fornece snapshot consistente de fila.
