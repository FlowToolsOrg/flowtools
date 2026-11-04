import { z } from 'zod'

// Configure before loading SDK/route modules: even Zod's caught eval probe
// produces a CSP violation. This retains its interpreter and validation rules.
z.config({ jitless: true })
