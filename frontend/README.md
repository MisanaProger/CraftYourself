# EST frontend

React + TypeScript + Vite. The UI only displays server state and starts use
cases through the API; business rules live in the Rust domain (architecture
v0.2, §13).

```sh
npm install
npm run dev      # http://localhost:5173, proxies /api to `est serve` on :7878
npm run build    # frontend/dist — served by `est serve` when present
```
