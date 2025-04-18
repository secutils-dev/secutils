import { useEffect } from 'react';
import { useLocation } from 'react-router';

// Height of the website fixed header.
const FIXED_HEADER_HEIGHT = 48;

export function useScrollToHash(delay = 250) {
  const location = useLocation();

  useEffect(() => {
    const elementToScroll = document.getElementById(location.hash.replace('#', ''));
    if (elementToScroll) {
      setTimeout(() => {
        window.scrollTo({ top: elementToScroll.offsetTop - FIXED_HEADER_HEIGHT, behavior: 'smooth' });
      }, delay);
    }
  }, []);
}
