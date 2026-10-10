[English](../../en/operations/milestone-1-review.md) | [README](../../../README.pt-BR.md)

# Revisão de fechamento do Marco 1

Data: 2026-10-08. Base revisada: `7cba38d` (Marco 1.7). Checkout inicialmente limpo, sem AGENTS.md aplicável. Mudanças de fechamento não commitadas; não se afirma novo hash, push ou CI remoto. Marco 1 **fechado nos critérios 1.1-1.8**. Sem bloqueadores de aceitação restantes. Marco 2 não iniciado.

## Achados e tratamento

Severidade média = semântica/status atual materialmente enganoso; baixa = melhoria de cobertura/navegação. Nenhum achado é defeito produtivo verificado.

| Achado | Severidade / tipo | Evidência na base revisada | Ação de fechamento |
| --- | --- | --- | --- |
| Roteiro ainda considera integração planejada apesar do 1.7 concluído; páginas tratam 1.8 como próximo | Média, inconsistência documental | Roteiro/progresso architecture.md; rodapés README/repositório/testes nos dois idiomas | Reconciliar implementado/planejado, fechar 1.8 documental, manter crash testing futuro e registros históricos |
| “Sem locks” sugere leitura sem qualquer lock | Média, semântica documental | Seção de leitura postgres-repository.md; SQL SELECT simples; ACCESS SHARE oficial PostgreSQL 18 | Distinguir ausência de locks de propriedade/claims de locks normais de tabela; esclarecer isolamento efetivo e snapshot |
| Histórico aplicado descrito incorretamente como ausente do diretório de dados | Média, semântica documental | Estado físico docker-and-configuration.md versus database-migrations.md e catálogo public._sqlx_migrations real | Separar SQL versionado de metadados aplicados no banco; explicitar limites de down destrutivo |
| JSON complexo comparado a outra leitura do driver sem assertion completa do esperado explícito | Baixa, cobertura de teste | restores_every_envelope_field_and_pending_metadata compara payload armazenado; folhas numéricas explícitas | Comparar fixture inteira com JSON esperado explícito, normalizando expoente; sem mudar decoder |
| Script de schema com 25 casos fora da suíte isolada de fechamento | Baixa, melhoria de validação | tests/sql/outbox_schema.sql tem BEGIN/ROLLBACK; harness Rust já isola bancos | Adicionar caso ignorado que executa fixture SQL inalterada e confere zero linhas restantes |

Código produtivo, dependências/lockfile Cargo, migrações, credenciais compartilhadas e dados da aplicação não mudaram. Sem camadas especulativas ou métodos writer/claim. Não houve decisão arquitetural material nova que exija ADR; ADRs 001-005 continuam aplicáveis como decisões históricas.

## Evidência de aceitação

| Critério | Resultado revisado / evidência |
| --- | --- |
| 1.1 Envelope | Campos privados, validação tipada, criação UUID v7; restore/Serde preservam identidade/tempo históricos; espaços não vazios preservados; seis testes |
| 1.2 PostgreSQL | Docker PostgreSQL 18.6 saudável, host loopback 5433, interno postgres:5432, bind mount persistente; diagnóstico somente leitura/autenticação passou |
| 1.3 Migrações | Dois pares up/down imutáveis; wrapper SQLx 0.8.6 codifica credenciais e fixa topologia interna; ambas instaladas, sem aplicação pendente necessária |
| 1.4 Schema | 15 colunas, 11 NOT NULL, chave primária, sete checks e índice parcial pending; 25 assertions versionadas executadas em isolamento com rollback |
| 1.5 Contrato | Corte UTC explícito, limite positivo, snapshots pending, três erros com fontes; futures Send nativas/dispatch estático; domínio/contratos sem SQLx; cinco testes |
| 1.6 Repositório | Um SELECT parametrizado limitado; decoders privados verificados; LIMIT verificado antes de I/O; sem escrita/claims/migração automática; sete testes unitários do adapter |
| 1.7 Integração | 15 casos opt-in atuais passaram com uma/quatro threads; pool fechado/limite excessivo dispensa banco; bancos gerados ausentes ao final |
| 1.8 Semântica | Documentação coordenada de produtor/leitura/falhas/durabilidade/recuperação, matriz de evidências e fontes oficiais PostgreSQL 18/SQLx 0.8.6; decisões futuras explícitas e handoff português |

Diferenças de representação são limites deliberados, não defeitos novos: INTEGER de tentativas termina em i32::MAX e modelo usa u32; escrita futura precisa checar largura. BIGINT com CHECK preserva versões u32 positivas. PostgreSQL aceita campos só com espaços, infinity e tempos fora do Chrono; linhas selecionadas incompatíveis falham corretamente. JSONB normaliza representação; precisão arbitrária preserva valor numérico armazenado. Restauração UTC preserva instante em microssegundos, não grafia de offset/nanossegundos. Identidade UUID não implica ordem de negócio.

## Revisão de responsabilidades e operação

Modelos controlam invariantes/conversões; infraestrutura controla SQL/mapeamento e depende para dentro. Health HTTP não tem orquestração de negócio a delegar; camadas controller/use-case/service de encaminhamento seriam prematuras. Controllers futuros delegam a casos de uso; casos de uso orquestram, negócio reutilizável fica em services, utilidades genéricas em helpers e mensageria externa em adapters. Nada foi adicionado somente para completar diagrama.

Pools injetados pertencem ao chamador. Harness usa nomes controlados/citados, migrações ordenadas, fixtures explícitas, prazos de aquisição/instrução/lock/caso/limpeza, barreira, erros sanitizados e limpeza após panic/erro. Dois jobs CI preservam verificações sem banco e usam mesmo target paralelo testado com PostgreSQL 18.6/credenciais descartáveis. Execução GitHub não validada. Cargo --locked preserva resolução; tags de ferramentas não tornam digest/base/OS imutáveis.

Prazos produtivos, admissão de workers, backup/restore, recuperação de crash abrupto, prazo de cancelamento, injeção de pool esgotado e toda sequência de escritas concorrentes seguem não validados/futuros. Bootstrap HTTP continua independente de readiness do banco e sem prazo forçado de shutdown. Isso não bloqueia escopo explícito do Marco 1 nem estabelece entrega funcional. Veja [semântica/decisões abertas](../architecture/failure-and-transactions.md), [validação executada](../testing/validation-results.md) e [handoff](handoff-marco-1.md).
