import { Container, Tabs } from '@mantine/core'
import { Notifications } from '@mantine/notifications'
import { WatermarkTab } from './components/WatermarkTab'
import { AnalyzeTab } from './components/AnalyzeTab'

function App() {
  return (
    <div style={{minWidth: "100vw"}}>
      <Notifications />
      <Container size="2xl" py="xl">
        <Tabs defaultValue="watermark">
          <Tabs.List>
            <Tabs.Tab value="watermark">Watermark</Tabs.Tab>
            <Tabs.Tab value="analyze">Analyze</Tabs.Tab>
          </Tabs.List>

          <Tabs.Panel value="watermark" pt="xl">
            <WatermarkTab />
          </Tabs.Panel>

          <Tabs.Panel value="analyze" pt="xl">
            <AnalyzeTab />
          </Tabs.Panel>
        </Tabs>
      </Container>
    </div>
  )
}

export default App