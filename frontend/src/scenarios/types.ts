export type StageState =
  'pending' | 'active' | 'done' | 'interrupted' | 'uncertain' | 'skipped' | 'preview'
export interface Step {
  label: string
  title: string
  explanation: string
  trace: string
  time: string
  state: StageState
  broker: string
  durable: string
  output: string
  activity?: boolean
}
export interface Scenario {
  id: string
  title: string
  description: string
  expected: string
  result: string
  outcome: string
  steps: Step[]
}
