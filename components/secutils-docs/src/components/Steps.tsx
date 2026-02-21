import React, { useCallback, useRef } from 'react';

import './Steps.scss';

interface Step {
  img: string;
  alt?: string;
  caption: React.ReactNode;
}

interface StepsProps {
  steps: Step[];
}

export default function Steps({ steps }: StepsProps): React.ReactElement {
  const dialogRef = useRef<HTMLDialogElement>(null);
  const dialogImgRef = useRef<HTMLImageElement>(null);

  const openLightbox = useCallback((src: string, alt: string) => {
    const dialog = dialogRef.current;
    const img = dialogImgRef.current;
    if (!dialog || !img) {
      return;
    }

    img.src = src;
    img.alt = alt;
    dialog.showModal();
  }, []);

  const closeLightbox = useCallback(() => {
    dialogRef.current?.close();
  }, []);

  const onBackdropClick = useCallback(
    (e: React.MouseEvent<HTMLDialogElement>) => {
      if (e.target === dialogRef.current) closeLightbox();
    },
    [closeLightbox],
  );

  return (
    <div className="su-steps">
      {steps.map((step, index) => {
        const alt = step.alt ?? typeof step.caption === 'string' ? step.caption as string : `Step ${index + 1}`;
        return (
          <div key={index} className="su-steps__step">
            <div className="su-steps__indicator">{index + 1}</div>
            <div className="su-steps__content">
              <img
                src={step.img}
                alt={alt}
                loading="lazy"
                className="su-steps__img"
                role="button"
                tabIndex={0}
                onClick={() => openLightbox(step.img, alt)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') openLightbox(step.img, alt);
                }}
              />
              <p className="su-steps__caption">{step.caption}</p>
            </div>
          </div>
        );
      })}

      <dialog ref={dialogRef} className="su-steps__lightbox" onClick={onBackdropClick}>
        <img ref={dialogImgRef} className="su-steps__lightbox-img" />
      </dialog>
    </div>
  );
}
