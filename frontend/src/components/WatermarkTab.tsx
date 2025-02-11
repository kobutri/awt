import { useState, useEffect } from 'react'
import { 
  Title, 
  Text, 
  Group, 
  Button, 
  Progress, 
  Card,
  Stack,
  Paper,
  useMantineTheme,
  Grid
} from '@mantine/core'
import { Dropzone } from '@mantine/dropzone'
import { notifications } from '@mantine/notifications'
import { FiUpload, FiX, FiVideo } from 'react-icons/fi'

interface ProcessingStatus {
  status: string;
  error: string | null;
}

export function WatermarkTab() {
  const [file, setFile] = useState<File | null>(null)
  const [uploading, setUploading] = useState(false)
  const [sessionId, setSessionId] = useState<string>('')
  const [status, setStatus] = useState<ProcessingStatus | null>(null)
  const [videoPreview, setVideoPreview] = useState<string>('')
  const theme = useMantineTheme()

  useEffect(() => {
    if (file) { 
      const url = URL.createObjectURL(file)
      setVideoPreview(url)
      return () => URL.revokeObjectURL(url)
    }
  }, [file])

  // Poll for status updates
  useEffect(() => {
    if (sessionId && (status?.status === 'uploading' || status?.status === 'processing')) {
      const interval = setInterval(async () => {
        try {
          const response = await fetch(`http://localhost:8000/status/${sessionId}`)
          const newStatus: ProcessingStatus = await response.json()
          setStatus(newStatus)
          
          if (newStatus.status === 'completed') {
            // Start download when processing is complete
            const downloadResponse = await fetch(`http://localhost:8000/download/${sessionId}`)
            if (!downloadResponse.ok) throw new Error('Download failed')

            const blob = await downloadResponse.blob()
            const url = window.URL.createObjectURL(blob)
            const a = document.createElement('a')
            a.href = url
            a.download = 'processed-video.mp4'
            document.body.appendChild(a)
            a.click()
            document.body.removeChild(a)
            window.URL.revokeObjectURL(url)

            notifications.show({
              title: 'Success',
              message: 'Video processing complete! Downloading now...',
              color: 'green'
            })
          } else if (newStatus.status === 'failed') {
            notifications.show({
              title: 'Error',
              message: newStatus.error || 'Processing failed',
              color: 'red',
              icon: <FiX size="1.1rem" />
            })
          }
        } catch (error) {
          console.error('Error checking status:', error)
        }
      }, 1000)

      return () => clearInterval(interval)
    }
  }, [sessionId, status])

  const handleDrop = (files: File[]) => {
    const videoFile = files[0]
    if (videoFile.type.startsWith('video/')) {
      setFile(videoFile)
    } else {
      notifications.show({
        title: 'Invalid File Type',
        message: 'Please upload a video file',
        color: 'red',
        icon: <FiX size="1.1rem" />
      })
    }
  }

  const handleUpload = async () => {
    if (!file) return

    setUploading(true)
    try {
      const formData = new FormData()
      formData.append('video', file)

      const response = await fetch('http://localhost:8000/upload', {
        method: 'POST',
        body: formData,
      })

      if (!response.ok) throw new Error('Upload failed')
      
      const sessionId = await response.text()
      setSessionId(sessionId)
      setStatus({ status: 'uploading', error: null })

      notifications.show({
        title: 'Upload Complete',
        message: 'Your video is being processed...',
        color: 'green'
      })
    } catch (error) {
      notifications.show({
        title: 'Error',
        message: error instanceof Error ? error.message : 'An error occurred',
        color: 'red',
        icon: <FiX size="1.1rem" />
      })
    } finally {
      setUploading(false)
    }
  }

  return (
    <Stack spacing="xl" w="100%" h="100%">
      <Paper shadow="sm" p="md" withBorder>
        <Title order={2} mb="md">Video Watermarking</Title>
        <Grid>
          <Grid.Col span={file ? 6 : 12}>
            <Dropzone
              onDrop={handleDrop}
              onReject={() => {
                notifications.show({
                  title: 'Invalid File Type',
                  message: 'Please upload a video file',
                  color: 'red',
                  icon: <FiX size="1.1rem" />
                })
              }}
              maxSize={100 * 1024 ** 3}
              accept={['video/*']}
              h={300}
            >
              <Stack align="center" justify="center" h="100%" spacing="xs">
                <Dropzone.Accept>
                  <FiUpload
                    size={240}
                    color={theme.colors[theme.primaryColor][6]}
                  />
                </Dropzone.Accept>
                <Dropzone.Reject>
                  <FiX
                    size={240}
                    color={theme.colors.red[6]}
                  />
                </Dropzone.Reject>
                <Dropzone.Idle>
                  <FiVideo size={240} />
                </Dropzone.Idle>
                <Text size="xl" inline>
                  Drag a video here or click to select
                </Text>
              </Stack>
            </Dropzone>
          </Grid.Col>
          
          {file && (
            <Grid.Col span={6}>
              <Card shadow="sm" p="md" h="100%">
                <Stack h="100%" justify="space-between">
                  <div>
                    <Text weight={500} size="lg" mb="xs">Preview</Text>
                    <video 
                      src={videoPreview} 
                      controls 
                      style={{ width: '100%', maxHeight: '1000px', objectFit: 'contain' }}
                    />
                    <Text size="sm" color="dimmed" mt="xs">
                      {file.name} ({(file.size / (1024 * 1024)).toFixed(2)} MB)
                    </Text>
                  </div>
                  <Button
                    onClick={handleUpload}
                    loading={uploading}
                    leftIcon={<FiUpload size="1.1rem" />}
                    fullWidth
                  >
                    {uploading ? 'Processing...' : 'Start Processing'}
                  </Button>
                </Stack>
              </Card>
            </Grid.Col>
          )}
        </Grid>
      </Paper>

      {(status?.status === 'uploading' || status?.status === 'processing') && (
        <Paper shadow="sm" p="md" withBorder>
          <Stack spacing="xs">
            <Text weight={500}>
              {status.status === 'uploading' ? 'Uploading...' : 'Processing video...'}
            </Text>
            <Progress
              value={status.status === 'uploading' ? 50 : 75}
              animate
              size="xl"
              radius="xl"
            />
          </Stack>
        </Paper>
      )}
    </Stack>
  )
}
