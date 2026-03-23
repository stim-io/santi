import 'dotenv/config'

import { baseUrl } from './env'

// Keep global setup intentionally thin.
// E2E should fail fast when the black-box target is not reachable,
// but environment loading itself should stay predictable and centralized.
baseUrl()
