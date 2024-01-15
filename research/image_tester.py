import cv2
import numpy as np

def classify_and_color_lines(edges, original_image):
    # Detect lines using Hough transform
    lines = cv2.HoughLinesP(edges, 1, np.pi / 180, threshold=50, minLineLength=50, maxLineGap=10)

    # Create a blank image that will be used to draw colored lines
    line_image = np.zeros_like(original_image)

    # Define colors
    colors = [(255, 0, 0), (0, 255, 0), (0, 0, 255)]  # Red, Green, Blue

    # Classify lines and draw them on the line_image
    for line in lines:
        for x1, y1, x2, y2 in line:
            angle = np.arctan2(y2 - y1, x2 - x1) * 180. / np.pi
            length = np.sqrt((y2 - y1)**2 + (x2 - x1)**2)

            # Determine the class of the line based on angle and length
            if angle > 45:
                line_class = 0  # Example class based on angle
            elif length > 100:
                line_class = 1  # Example class based on length
            else:
                line_class = 2  # Default class

            # Draw the line with the color corresponding to the class
            cv2.line(line_image, (x1, y1), (x2, y2), colors[line_class], 2)

    # Combine the line image with the original image
    colored_lines_image = cv2.addWeighted(original_image, 0.8, line_image, 1, 0)

    return colored_lines_image

# Your existing process_frame function
def process_frame(frame_data):
    # Your existing code to generate 'edges' from the frame

    # Call the classify_and_color_lines function
    colored_frame = classify_and_color_lines(edges, frame)

    # Continue with any additional processing, resizing, adding text, etc.
