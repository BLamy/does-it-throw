import { Something, someObjectLiteral } from './something'

const someRandomThrow = () => {
  throw new Error('some random throw')
}

const server = http.createServer(async (req, res) => {
  switch (req.url) {
    case '/api/pong':
      console.log('pong!')
      throw new Error('')
      break
    case '/api/ping':
      console.log('ping!')
      const ips = await Something()
      someObjectLiteral.objectLiteralThrow()
      const others = ips.filter((ip) => ip !== "localhost")

      others.forEach((ip) => {
        fetch(`http://[${ip}]:8080/api/pong`)
      })
      break
    case '/api/throw':
      someRandomThrow()
      break
  }

  res.end()
})

const wss = new WebSocketServer({ noServer: true })
