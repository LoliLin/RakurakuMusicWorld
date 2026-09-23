import { createBrowserRouter, createHashRouter } from 'react-router-dom'
import App from './App'
import PlayerPage from './pages/PlayerPage'
import LibraryPage from './pages/LibraryPage'
import SettingsPage from './pages/SettingsPage'
import ErrorPage from './pages/ErrorPage'

/**
 * The player is the home page (hosts the "listen together" presence and the
 * shared queue); admin capabilities live inline — skip/remove in the player
 * and queue, the rest under Settings → 电台管理.
 */
const isFile = typeof window !== 'undefined' && window.location.protocol === 'file:'
const createRouter = isFile ? createHashRouter : createBrowserRouter

export const router = createRouter(
  [
    {
      path: '/',
      element: <App />,
      errorElement: <ErrorPage />,
      children: [
        { index: true, element: <PlayerPage /> },
        { path: 'player', element: <PlayerPage /> },
        { path: 'library', element: <LibraryPage /> },
        { path: 'settings', element: <SettingsPage /> },
      ],
    },
  ],
  isFile ? {} : { basename: import.meta.env.BASE_URL.replace(/\/$/, '') },
)
