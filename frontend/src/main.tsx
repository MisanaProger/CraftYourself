import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Providers } from "./app/providers";
import { AppRouter } from "./app/router";
import { ThemeProvider } from "./app/theme";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ThemeProvider>
      <Providers>
        <AppRouter />
      </Providers>
    </ThemeProvider>
  </StrictMode>,
);
