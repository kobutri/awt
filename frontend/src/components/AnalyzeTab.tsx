import { Paper, FileInput, Text, Stack, Group, Box, Table, Accordion, Button, Grid, Title, Card } from '@mantine/core'
import { FiVideo, FiSearch } from 'react-icons/fi'
import { useEffect, useState } from 'react'
import { createC2pa, selectProducer } from 'c2pa'
// @ts-ignore
import wasmSrc from 'c2pa/dist/assets/wasm/toolkit_bg.wasm?url'
// @ts-ignore
import workerSrc from 'c2pa/dist/c2pa.worker.min.js?url'

interface ManifestData {
  metadata: Record<string, string>
  assertions: Array<{
    label: string
    data: any
  }>
}

interface VideoData {
  path: string
  manifest_json: string
  message_bits: number[]
}

export function AnalyzeTab() {
  const [videoFile, setVideoFile] = useState<File | null>(null)
  const [videoUrl, setVideoUrl] = useState<string>('')
  const [uploadedManifestData, setUploadedManifestData] = useState<ManifestData | null>(null)
  const [analyzedManifestData, setAnalyzedManifestData] = useState<ManifestData | null>(null)
  const [error, setError] = useState<string>('')
  const [matchedVideoUrl, setMatchedVideoUrl] = useState<string>('')
  const [isSearching, setIsSearching] = useState(false)

  useEffect(() => {
    if (videoFile) {
      const url = URL.createObjectURL(videoFile)
      setVideoUrl(url)
      
      // Initialize C2PA and extract manifest
      createC2pa({
        wasmSrc,
        workerSrc,
      })
        .then((c2pa) => c2pa.read(videoFile))
        .then(({ manifestStore, source }) => {
          const activeManifest = manifestStore?.activeManifest
          if (activeManifest) {
            // Get metadata properties
            const metadata: Record<string, string> = {
              'Title': activeManifest.title || 'Untitled',
              'Format': activeManifest.format || 'Unknown',
              'Generator': (activeManifest.claimGenerator || '').split('(')[0]?.trim() || 'Unknown',
              'Producer': selectProducer(activeManifest)?.name || 'Unknown',
              'Ingredients': (activeManifest.ingredients || [])
                .map((i) => i.title)
                .join(', ') || 'None',
              'Signature Issuer': activeManifest.signatureInfo?.issuer || 'Unknown',
              'Signature Date': activeManifest.signatureInfo?.time
                ? new Date(activeManifest.signatureInfo.time).toLocaleString()
                : 'No date available',
            }

            // Get assertions
            const assertions = activeManifest.assertions.data.map(assertion => ({
              label: assertion.label,
              data: assertion.data
            }))

            setUploadedManifestData({ metadata, assertions })
            setError('')
          } else {
            setError('No C2PA manifest found in the video')
            setUploadedManifestData(null)
          }
        })
        .catch((err) => {
          setError('Failed to extract C2PA data: ' + err.message)
          setUploadedManifestData(null)
        })

      return () => URL.revokeObjectURL(url)
    }
  }, [videoFile])

  const handleSearchVideo = async () => {
    if (!videoFile) {
      setError('Please select a video file first')
      return
    }

    setIsSearching(true)
    setError('')
    setMatchedVideoUrl('')
    setAnalyzedManifestData(null)

    const formData = new FormData()
    formData.append('video', videoFile)

    try {
      const response = await fetch('http://localhost:8000/analyze', {
        method: 'POST',
        body: formData,
      })

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`)
      }

      // Create a blob URL from the response
      const blob = await response.blob()
      const url = URL.createObjectURL(blob)
      setMatchedVideoUrl(url)

      // Extract C2PA manifest from the analyzed video
      const analyzedFile = new File([blob], 'analyzed.mp4', { type: 'video/mp4' })
      createC2pa({
        wasmSrc,
        workerSrc,
      })
        .then((c2pa) => c2pa.read(analyzedFile))
        .then(({ manifestStore, source }) => {
          const activeManifest = manifestStore?.activeManifest
          if (activeManifest) {
            // Get metadata properties
            const metadata: Record<string, string> = {
              'Title': activeManifest.title || 'Untitled',
              'Format': activeManifest.format || 'Unknown',
              'Generator': (activeManifest.claimGenerator || '').split('(')[0]?.trim() || 'Unknown',
              'Producer': selectProducer(activeManifest)?.name || 'Unknown',
              'Ingredients': (activeManifest.ingredients || [])
                .map((i) => i.title)
                .join(', ') || 'None',
              'Signature Issuer': activeManifest.signatureInfo?.issuer || 'Unknown',
              'Signature Date': activeManifest.signatureInfo?.time
                ? new Date(activeManifest.signatureInfo.time).toLocaleString()
                : 'No date available',
            }

            // Get assertions
            const assertions = activeManifest.assertions.data.map(assertion => ({
              label: assertion.label,
              data: assertion.data
            }))

            setAnalyzedManifestData({ metadata, assertions })
            setError('')
          } else {
            setError('No C2PA manifest found in the analyzed video')
            setAnalyzedManifestData(null)
          }
        })
        .catch((err) => {
          setError('Failed to extract C2PA data from analyzed video: ' + err.message)
          setAnalyzedManifestData(null)
        })

      setIsSearching(false)
    } catch (error) {
      console.error('Error:', error)
      setError('Failed to analyze video')
      setIsSearching(false)
    }
  };

  return (
    <Stack spacing="xl" w="100%" h="100%">
      {error && (
        <Paper shadow="sm" p="md" withBorder>
          <Text color="red" weight={500}>{error}</Text>
        </Paper>
      )}

      <Grid gutter="xl">
        {/* Left Column */}
        <Grid.Col span={6}>
          <Stack spacing="md">
            {/* Upload Video Section */}
            <Paper shadow="sm" p="md" withBorder>
              <Stack spacing="md">
                <Title order={3}>Original Video</Title>
                {videoFile ? (
                  <>
                    <video 
                      src={videoUrl} 
                      controls 
                      style={{ width: '100%', maxHeight: '300px', objectFit: 'contain' }}
                    />
                    <Text size="sm" color="dimmed">
                      {videoFile.name} ({(videoFile.size / (1024 * 1024)).toFixed(2)} MB)
                    </Text>
                  </>
                ) : (
                  <FileInput
                    label="Upload Video"
                    placeholder="Choose a video file"
                    accept="video/*"
                    value={videoFile}
                    onChange={setVideoFile}
                    size="md"
                    icon={<FiVideo size="1.1rem" />}
                  />
                )}
              </Stack>
            </Paper>

            {/* Analyze Button */}
            {videoFile && (
              <Button
                onClick={handleSearchVideo}
                loading={isSearching}
                leftIcon={<FiSearch size="1.1rem" />}
                size="md"
              >
                {isSearching ? 'Analyzing...' : 'Analyze Video'}
              </Button>
            )}

            {/* Original Video Manifest Data */}
            {uploadedManifestData && (
              <Paper shadow="sm" p="md" withBorder>
                <Stack spacing="md">
                  <Title order={3}>Original Video Manifest</Title>
                  <Card withBorder>
                    <Text weight={500} mb="md">Metadata</Text>
                    <Table>
                      <tbody>
                        {Object.entries(uploadedManifestData.metadata).map(([key, value]) => (
                          <tr key={key}>
                            <td style={{ fontWeight: 500 }}>{key}</td>
                            <td>{value}</td>
                          </tr>
                        ))}
                      </tbody>
                    </Table>
                  </Card>
                  <Card withBorder>
                    <Text weight={500} mb="md">Assertions</Text>
                    <Accordion>
                      {uploadedManifestData.assertions.map((assertion, index) => (
                        <Accordion.Item key={index} value={assertion.label}>
                          <Accordion.Control>{assertion.label}</Accordion.Control>
                          <Accordion.Panel>
                            <pre style={{ whiteSpace: 'pre-wrap', fontSize: '0.9em' }}>
                              {JSON.stringify(assertion.data, null, 2)}
                            </pre>
                          </Accordion.Panel>
                        </Accordion.Item>
                      ))}
                    </Accordion>
                  </Card>
                </Stack>
              </Paper>
            )}
          </Stack>
        </Grid.Col>

        {/* Right Column */}
        <Grid.Col span={6}>
          {analyzedManifestData && (
            <Stack spacing="md">
              {/* Analyzed Video */}
              <Paper shadow="sm" p="md" withBorder>
                <Stack spacing="md">
                  <Title order={3}>Analyzed Video</Title>
                  {matchedVideoUrl ? (
                    <video 
                      src={matchedVideoUrl} 
                      controls 
                      style={{ width: '100%', maxHeight: '300px', objectFit: 'contain' }}
                    />
                  ) : (
                    <Text color="dimmed">No match found</Text>
                  )}
                </Stack>
              </Paper>

              {/* Analyzed Video Manifest Data */}
              <Paper shadow="sm" p="md" withBorder>
                <Stack spacing="md">
                  <Title order={3}>Analyzed Video Manifest</Title>
                  <Card withBorder>
                    <Text weight={500} mb="md">Metadata</Text>
                    <Table>
                      <tbody>
                        {Object.entries(analyzedManifestData.metadata).map(([key, value]) => (
                          <tr key={key}>
                            <td style={{ fontWeight: 500 }}>{key}</td>
                            <td>{value}</td>
                          </tr>
                        ))}
                      </tbody>
                    </Table>
                  </Card>
                  <Card withBorder>
                    <Text weight={500} mb="md">Assertions</Text>
                    <Accordion>
                      {analyzedManifestData.assertions.map((assertion, index) => (
                        <Accordion.Item key={index} value={assertion.label}>
                          <Accordion.Control>{assertion.label}</Accordion.Control>
                          <Accordion.Panel>
                            <pre style={{ whiteSpace: 'pre-wrap', fontSize: '0.9em' }}>
                              {JSON.stringify(assertion.data, null, 2)}
                            </pre>
                          </Accordion.Panel>
                        </Accordion.Item>
                      ))}
                    </Accordion>
                  </Card>
                </Stack>
              </Paper>
            </Stack>
          )}
        </Grid.Col>
      </Grid>
    </Stack>
  )
}
